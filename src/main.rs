mod startup;
use std::{io::{Cursor,Write},net::{IpAddr,Ipv4Addr,SocketAddr},sync::{Arc,Mutex}};
use anyhow::{anyhow,Result};
use axum::{extract::{DefaultBodyLimit,Multipart,State},http::{header,HeaderMap,StatusCode},response::{Html,IntoResponse,Response},routing::{delete,get,post},Json,Router};
use image::{ImageFormat,ImageReader,ImageDecoder};
use serde::Serialize;
use zip::{write::FileOptions,ZipWriter};

const MAX_FILE:usize=5*1024*1024;
const MAX_PIXELS:u64=16_000_000;
#[derive(Clone,Serialize)]
struct Item {name:String,status:String,error:Option<String>,input_bytes:usize,output_bytes:Option<usize>}
#[derive(Clone,Serialize)]
struct View {running:bool,items:Vec<Item>,ready:bool}
#[derive(Default)]
struct Job {running:bool,items:Vec<Item>,archive:Option<Vec<u8>>}
type App=Arc<Mutex<Job>>;
fn change(app:&App,i:usize,status:&str,error:Option<String>){if let Ok(mut job)=app.lock(){if let Some(item)=job.items.get_mut(i){item.status=status.into();item.error=error;}}}
fn process(bytes:&[u8],w:u32,h:u32,format:ImageFormat,quality:u8)->Result<Vec<u8>>{
 let reader=ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
 let fmt=reader.format().ok_or_else(||anyhow!("Unsupported image"))?;
 anyhow::ensure!(matches!(fmt,ImageFormat::Jpeg|ImageFormat::Png|ImageFormat::WebP),"Only JPEG, PNG and WebP are supported");
 let mut decoder=reader.into_decoder()?;
 let (width,height)=decoder.dimensions();
 anyhow::ensure!(width>0 && height>0 && u64::from(width)*u64::from(height)<=MAX_PIXELS,"Image exceeds pixel limit");
 let mut limits=image::Limits::default();limits.max_alloc=Some(128*1024*1024);decoder.set_limits(limits)?;
 let orientation=decoder.orientation()?;
 let mut img=image::DynamicImage::from_decoder(decoder)?;
 img.apply_orientation(orientation);
 let resized=img.resize(w,h,image::imageops::FilterType::Lanczos3);
 let mut out=Cursor::new(Vec::new());
 if format==ImageFormat::Jpeg {image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out,quality).encode_image(&image::DynamicImage::ImageRgb8(resized.to_rgb8()))?;} else {resized.write_to(&mut out,format)?;}
 Ok(out.into_inner())
}
struct Admission{app:App,transferred:bool}
impl Drop for Admission{fn drop(&mut self){if !self.transferred{if let Ok(mut j)=self.app.lock(){j.running=false;}}}}
async fn create(State(app):State<App>,headers:HeaderMap,mut multipart:Multipart)->Result<StatusCode,StatusCode>{
 if headers.get("x-forge-request").and_then(|x|x.to_str().ok())!=Some("1"){return Err(StatusCode::FORBIDDEN)}
 {let mut job=app.lock().map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;if job.running{return Err(StatusCode::CONFLICT)}job.running=true;job.items.clear();job.archive=None;}
 let mut admission=Admission{app:app.clone(),transferred:false};
 let parsed=tokio::time::timeout(std::time::Duration::from_secs(30),async {
  let mut files:Vec<(String,Vec<u8>)>=Vec::new();let mut width=1200u32;let mut height=1200u32;let mut format=String::from("webp");let mut quality=85u8;
  while let Some(field)=multipart.next_field().await.map_err(|_|StatusCode::BAD_REQUEST)? {
   let name=field.name().unwrap_or("").to_string();
   if name=="files" {if files.len()>=10{return Err(StatusCode::PAYLOAD_TOO_LARGE)}let filename=field.file_name().unwrap_or("image").chars().take(100).collect::<String>();let bytes=field.bytes().await.map_err(|_|StatusCode::BAD_REQUEST)?;if bytes.len()>MAX_FILE{return Err(StatusCode::PAYLOAD_TOO_LARGE)}files.push((filename,bytes.to_vec()));}
   else if ["width","height","format","quality"].contains(&name.as_str()){let value=field.text().await.map_err(|_|StatusCode::BAD_REQUEST)?;match name.as_str(){"width"=>width=value.parse().map_err(|_|StatusCode::BAD_REQUEST)?,"height"=>height=value.parse().map_err(|_|StatusCode::BAD_REQUEST)?,"format"=>format=value,"quality"=>quality=value.parse().map_err(|_|StatusCode::BAD_REQUEST)?,_=>{}}}
  }
  if files.is_empty()||!(1..=4000).contains(&width)||!(1..=4000).contains(&height)||!(1..=100).contains(&quality)||!matches!(format.as_str(),"jpg"|"png"|"webp"){return Err(StatusCode::BAD_REQUEST)}
  Ok((files,width,height,format,quality))
 }).await.unwrap_or(Err(StatusCode::REQUEST_TIMEOUT));
 let (files,width,height,fmt,quality)=match parsed{Ok(v)=>v,Err(e)=>{if let Ok(mut job)=app.lock(){job.running=false}return Err(e)}};
 {let mut job=app.lock().map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;job.items=files.iter().map(|(name,bytes)|Item{name:name.clone(),status:"queued".into(),error:None,input_bytes:bytes.len(),output_bytes:None}).collect();}
 admission.transferred=true;
 let app2=app.clone();tokio::spawn(async move {
  let format=match fmt.as_str(){"jpg"=>ImageFormat::Jpeg,"png"=>ImageFormat::Png,_=>ImageFormat::WebP};
  let handle=tokio::task::spawn_blocking(move||{
   let mut archive=ZipWriter::new(Cursor::new(Vec::new()));let options=FileOptions::default().compression_method(zip::CompressionMethod::Deflated);let mut count=0;
   for (i,(_,bytes)) in files.into_iter().enumerate(){change(&app2,i,"processing",None);match process(&bytes,width,height,format,quality){
    Ok(data)=>{let output_len=data.len();let filename=format!("image-{:03}.{}",i+1,fmt);match archive.start_file(filename,options).and_then(|_|archive.write_all(&data).map_err(zip::result::ZipError::Io)){Ok(())=>{if let Ok(mut job)=app2.lock(){if let Some(item)=job.items.get_mut(i){item.status="completed".into();item.error=None;item.output_bytes=Some(output_len);}}count+=1},Err(_)=>{change(&app2,i,"failed",Some("Could not write archive".into()));break}}},
    Err(e)=>change(&app2,i,"failed",Some(e.to_string()))
   }}
   let result=if count>0 {archive.finish().ok().map(|c|c.into_inner())}else{None};
   if let Ok(mut job)=app2.lock(){job.archive=result;job.running=false;for item in &mut job.items {if item.status=="queued"||item.status=="processing"{item.status="failed".into();item.error=Some("Processing interrupted".into());}}}
  });
  if handle.await.is_err(){if let Ok(mut job)=app.lock(){job.running=false;for i in &mut job.items{if i.status=="queued"||i.status=="processing"{i.status="failed".into();i.error=Some("Processing interrupted".into());}}}}
 });
 Ok(StatusCode::ACCEPTED)
}
async fn state(State(app):State<App>)->Result<Json<View>,StatusCode>{let j=app.lock().map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;Ok(Json(View{running:j.running,items:j.items.clone(),ready:j.archive.is_some()}))}
async fn download(State(app):State<App>)->Response{let data=app.lock().ok().and_then(|j|j.archive.clone());match data{Some(bytes)=>([(header::CONTENT_TYPE,"application/zip"),(header::CONTENT_DISPOSITION,"attachment; filename=forge-results.zip")],bytes).into_response(),None=>StatusCode::NOT_FOUND.into_response()}}
async fn clear(State(app):State<App>,headers:HeaderMap)->StatusCode{if headers.get("x-forge-request").and_then(|v|v.to_str().ok())!=Some("1"){return StatusCode::FORBIDDEN}match app.lock(){Ok(mut j)=>{if j.running{StatusCode::CONFLICT}else{*j=Job::default();StatusCode::NO_CONTENT}},Err(_)=>StatusCode::INTERNAL_SERVER_ERROR}}
async fn index()->Html<&'static str>{Html(include_str!("../web/index.html"))}
async fn css()->impl IntoResponse{([(header::CONTENT_TYPE,"text/css; charset=utf-8")],include_str!("../web/style.css"))}
async fn js()->impl IntoResponse{([(header::CONTENT_TYPE,"text/javascript; charset=utf-8")],include_str!("../web/app.js"))}
#[tokio::main]
async fn main()->Result<()> {if std::env::args().any(|a|a=="--check"){return startup::check(None,Some(3003));}let app:App=Arc::new(Mutex::new(Job::default()));let router=Router::new().route("/",get(index)).route("/style.css",get(css)).route("/app.js",get(js)).route("/api/job",get(state).post(create).delete(clear)).route("/api/download",get(download)).layer(DefaultBodyLimit::max(52*1024*1024)).with_state(app);let addr=SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST),3003);let listener=tokio::net::TcpListener::bind(addr).await.map_err(|_|anyhow::anyhow!("Cannot bind local port; stop the other instance / Локальный порт занят или недоступен"))?;println!("Forge Queue: http://{addr}");axum::serve(listener,router).with_graceful_shutdown(async{let _=tokio::signal::ctrl_c().await;}).await?;Ok(())}
#[cfg(test)]
mod tests {use super::*;
 #[test] fn resize_and_decode() -> Result<()> {let original=image::DynamicImage::new_rgb8(100,50);let mut input=Cursor::new(Vec::new());original.write_to(&mut input,ImageFormat::Png)?;for format in [ImageFormat::Png,ImageFormat::Jpeg,ImageFormat::WebP] {let result=process(input.get_ref(),40,40,format,85)?;assert_eq!(image::guess_format(&result)?,format);let decoded=image::load_from_memory(&result)?;assert_eq!(decoded.width(),40);assert_eq!(decoded.height(),20);}assert!(process(b"not an image",40,40,ImageFormat::Png,85).is_err());Ok(())}
}

#[cfg(test)]mod orientation_tests{use super::*;use anyhow::Context;
 fn jpeg_with_orientation(jpeg:&[u8],value:u16)->Vec<u8>{let mut exif=b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();exif.extend(value.to_le_bytes());exif.extend([0u8;6]);let mut out=jpeg[..2].to_vec();out.extend([255,225]);out.extend(((exif.len()+2) as u16).to_be_bytes());out.extend(exif);out.extend(&jpeg[2..]);out}
 #[test]fn exif_rotation_and_mirroring_apply_once()->Result<()>{let mut image=image::RgbImage::new(4,2);for (x,y,p)in image.enumerate_pixels_mut(){*p=image::Rgb([(x*60)as u8,(y*200)as u8,0]);}let mut input=Cursor::new(Vec::new());image::DynamicImage::ImageRgb8(image).write_to(&mut input,ImageFormat::Jpeg)?;let jpeg=input.into_inner();let base=image::load_from_memory(&jpeg)?.to_rgb8();for orientation in 1..=8{let bytes=jpeg_with_orientation(&jpeg,orientation);let result=process(&bytes,4,4,ImageFormat::Png,85)?;let decoded=image::load_from_memory(&result)?;let mut expected=image::DynamicImage::ImageRgb8(base.clone());expected.apply_orientation(image::metadata::Orientation::from_exif(orientation as u8).context("orientation")?);let expected=expected.resize(4,4,image::imageops::FilterType::Lanczos3);assert_eq!(decoded.to_rgb8(),expected.to_rgb8());let again=process(&result,4,4,ImageFormat::Png,85)?;assert_eq!(decoded.to_rgb8(),image::load_from_memory(&again)?.to_rgb8());}Ok(())}
 #[test]fn dropped_upload_releases_admission(){let app=Arc::new(Mutex::new(Job{running:true,..Default::default()}));{let _guard=Admission{app:app.clone(),transferred:false};}assert!(!app.lock().expect("job").running);}
}
