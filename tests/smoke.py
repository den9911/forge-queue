import base64, io, json, pathlib, subprocess, tempfile, time, urllib.request, zipfile
binary=pathlib.Path('target/release/forge-queue').resolve()
def request(url, data=None, headers=None, method=None):
    req=urllib.request.Request('http://127.0.0.1:3003'+url,data=data,headers=headers or {},method=method)
    with urllib.request.urlopen(req,timeout=10) as r:return r.read()
with tempfile.TemporaryDirectory() as directory:
    p=subprocess.Popen([str(binary)],cwd=directory,stdout=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try: request('/api/job');break
            except OSError:time.sleep(.1)
        boundary='forge-boundary-test'
        png=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGP8L8LAwMDAxMDAwMDAAAAODQEX4xf9HAAAAABJRU5ErkJggg==')
        def field(name,value):return (f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n').encode()
        def file(name,data):return (f'--{boundary}\r\nContent-Disposition: form-data; name="files"; filename="{name}"\r\nContent-Type: image/png\r\n\r\n').encode()+data+b'\r\n'
        body=field('width','100')+field('height','100')+field('format','png')+file('good.png',png)+file('bad.png',b'broken')+f'--{boundary}--\r\n'.encode()
        request('/api/job',body,{'Content-Type':'multipart/form-data; boundary='+boundary,'X-Forge-Request':'1'})
        for _ in range(100):
            job=json.loads(request('/api/job'))
            if not job['running']:break
            time.sleep(.1)
        assert [x['status'] for x in job['items']]==['completed','failed'],job
        data=request('/api/download');z=zipfile.ZipFile(io.BytesIO(data));assert z.namelist()==['image-001.png'];assert z.read('image-001.png').startswith(b'\x89PNG\r\n\x1a\n')
        request('/api/job',headers={'X-Forge-Request':'1'},method='DELETE');assert json.loads(request('/api/job'))['items']==[]
    finally:p.terminate();p.wait(timeout=5)
print('PASS: mixed image batch, ZIP and clearing via HTTP')
