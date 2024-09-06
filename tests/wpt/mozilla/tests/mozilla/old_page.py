import time

def main(request, response):
    response.headers.set(b"Age", b"90000")
    response.headers.set(b"Last-Modified", b"Wed, 21 Oct 2015 07:28:00 GMT")
    response.write_status_headers()
    response.writer.write_content(b"Body")
