"""Real TCP/HTTP regression for admission, input limits and no mixed batches."""
import binascii, http.client, io, json, pathlib, socket, struct, subprocess, tempfile, time, zipfile, zlib
binary=pathlib.Path('target/release/forge-queue').resolve()
boundary='recovery-boundary'
headers={'X-Forge-Request':'1','Content-Type':'multipart/form-data; boundary='+boundary}
def request(method='GET',path='/api/job',body=None):
    c=http.client.HTTPConnection('127.0.0.1',3003,timeout=10)
    try:
        c.request(method,path,body,headers if method!='GET' else {})
        r=c.getresponse();return r.status,r.read()
    finally:c.close()
def state():return json.loads(request()[1])
def wait(predicate,seconds=8):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        current=state()
        if predicate(current):return current
        time.sleep(.05)
    raise AssertionError('State did not recover')
def chunk(name,data):return struct.pack('>I',len(data))+name+data+struct.pack('>I',binascii.crc32(name+data)&0xffffffff)
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,2,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0'+b'\xff\0\0'*2+b'\0'+b'\xff\0\0'*2))+chunk(b'IEND',b'')
def prefix(name='good.png'):return f'--{boundary}\r\nContent-Disposition: form-data; name="files"; filename="{name}"\r\nContent-Type: image/png\r\n\r\n'.encode()
def batch(data=png,name='good.png'):return prefix(name)+data+f'\r\n--{boundary}--\r\n'.encode()
def partial(data):
    s=socket.create_connection(('127.0.0.1',3003),timeout=40)
    s.sendall((f'POST /api/job HTTP/1.1\r\nHost: localhost\r\nX-Forge-Request: 1\r\nContent-Type: multipart/form-data; boundary={boundary}\r\nContent-Length: 1048576\r\n\r\n').encode()+data)
    wait(lambda j:j['running']);return s
with tempfile.TemporaryDirectory() as directory:
    p=subprocess.Popen([str(binary)],cwd=directory,stdout=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:state();break
            except OSError:time.sleep(.1)
        for data in [b'--'+boundary.encode()+b'\r\n',prefix()+png[:12]]:
            s=partial(data)
            assert request('POST',body=batch())[0]==409
            assert request('DELETE')[0]==409
            s.close();j=wait(lambda j:not j['running']);assert j['items']==[] and not j['ready']
        print('PASS actual disconnect before first field and inside file; concurrent admission rejected')
        s=partial(prefix()+png[:12]);wait(lambda j:not j['running'],36);s.close()
        assert state()['items']==[];print('PASS real 30-second multipart timeout releases admission')
        assert request('POST',body=batch())[0]==202
        j=wait(lambda j:not j['running']);assert [x['status'] for x in j['items']]==['completed'],j
        status,archive=request(path='/api/download');assert status==200
        with zipfile.ZipFile(io.BytesIO(archive)) as z:
            assert z.namelist()==['image-001.webp'];assert z.read(z.namelist()[0])[:4]==b'RIFF'
        # An interrupted replacement preserves the previous successful result.
        s=partial(prefix('replacement.png')+png[:10]);s.close();j=wait(lambda j:not j['running']);assert j['ready'] and j['items'][0]['name']=='good.png';assert request(path='/api/download')[1]==archive
        assert request('POST',body=batch(b'x'*(5*1024*1024+1)))[0]==413
        assert request(path='/api/download')[1]==archive
        oversized=f'--{boundary}\r\nContent-Disposition: form-data; name="width"\r\n\r\n'.encode()+b'1'*65+f'\r\n--{boundary}--\r\n'.encode()
        assert request('POST',body=oversized)[0]==413
        assert request('DELETE')[0]==204
        assert request('POST',body=batch(name='next.png'))[0]==202
        j=wait(lambda j:not j['running']);assert len(j['items'])==1 and j['items'][0]['name']=='next.png'
        print('PASS bounded fields/files, prior ZIP preservation and independent next batch')
    finally:p.terminate();p.wait(timeout=5)
