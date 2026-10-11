"""Servidor de desarrollo: sirve ui/ con un simulador del nucleo (dev/shim.js) para ver la interfaz en un navegador.
La carpeta theme sale de la instalacion real (%APPDATA%/ZeresLauncher/theme), asi que hay que abrir el launcher una vez."""
import http.server, os, sys
ROOT = os.path.dirname(os.path.abspath(__file__))
UI = os.path.join(ROOT, "..", "ui")
THEME = os.path.join(os.environ["APPDATA"], "ZeresLauncher", "theme")

class H(http.server.SimpleHTTPRequestHandler):
    def translate_path(self, path):
        p = path.split("?")[0]
        if p.startswith("/theme/"):
            return os.path.join(THEME, *p[len("/theme/"):].split("/"))
        if p == "/dev-shim.js":
            return os.path.join(ROOT, "shim.js")
        return os.path.join(UI, *p.lstrip("/").split("/")) if p != "/" else os.path.join(UI, "index.html")
    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()
    def do_GET(self):
        if self.path.split("?")[0] in ("/", "/index.html"):
            html = open(os.path.join(UI, "index.html"), encoding="utf-8").read()
            html = html.replace('<script src="mcfont.js">', '<script src="/dev-shim.js"></script><script src="mcfont.js">')
            b = html.encode()
            self.send_response(200); self.send_header("Content-Type", "text/html; charset=utf-8"); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
            return
        super().do_GET()

http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1]) if len(sys.argv) > 1 else 8770), H).serve_forever()
