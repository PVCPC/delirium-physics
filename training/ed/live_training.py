"""Read-only, loopback browser monitor of actual PPO rollout observations."""
import argparse
from collections import deque
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import secrets
import math
from pathlib import Path
import socket
import struct
import threading
import time
from training_control import TrainingControl


PARENTS = [-1, 0, 1, 2, 0, 4, 5, 0, 7, 8, 9, 10, 11, 10, 13, 14, 15, 10, 17, 18, 19]


def body_frame(row):
    # learn::body_obs: height, root axis, 21 x (relative position/velocity/axis), root velocity x .2.
    if len(row) < 196 or not all(math.isfinite(float(x)) for x in row[:196]):
        raise ValueError('Invalid native body observation')
    points = [[float(row[4 + 9 * k + i]) + (float(row[0]) if i == 2 else 0.)
               for i in range(3)] for k in range(21)]
    speed = math.sqrt(sum(float(x) ** 2 for x in row[193:196])) * 5
    return points, speed


class LivePreview:
    """Best-effort telemetry; a closed browser/server never blocks or stops training."""
    def __init__(self, port, run):
        self.address = ('127.0.0.1', port)
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.socket.setblocking(False)
        self.run = str(Path(run).resolve())
        self.last_sent = 0.
        self.sequence = self.resets = 0
        self.elapsed = 0.
        self.previous_speed = None
        self.previous_sequence = 0
        self.frame = None
        self.dropped = 0

    def send(self, phase, update, force=False):
        now = time.monotonic()
        if not force and now - self.last_sent < 1 / 20:
            return
        payload = dict(run=self.run, phase=phase, update=update, sequence=self.sequence,
                       sent_at=time.time(), frame=self.frame, dropped=self.dropped)
        try:
            self.socket.sendto(json.dumps(payload, allow_nan=False, separators=(',', ':')).encode(), self.address)
        except OSError:
            self.dropped += 1
        self.last_sent = now

    def step(self, row, obs, update):
        reset = bool(row[obs + 1])
        self.elapsed = 0. if reset else self.elapsed + 1 / 30
        self.resets += int(reset)
        if reset:
            self.previous_speed = None
        self.sequence += 1
        if time.monotonic() - self.last_sent < 1 / 20:
            return
        points, speed = body_frame(row)
        acceleration = 0. if self.previous_speed is None else (speed - self.previous_speed) * 30 / (self.sequence - self.previous_sequence)
        self.previous_speed, self.previous_sequence = speed, self.sequence
        self.frame = dict(points=points, speed=speed, acceleration=0. if reset else acceleration,
                          trial_seconds=self.elapsed, resets=self.resets, captured_at=time.time())
        self.send('collecting', update)

    def replay(self, rows, obs, update):
        # A whole rollout's frames at once (--native-rollout): played at real time, a newer one cutting in.
        self.queue = (rows, obs, update)
        if not getattr(self, 'player', None):
            self.player = threading.Thread(target=self.play, daemon=True); self.player.start()

    def play(self):
        while True:
            item, self.queue = self.queue, None  # ponytail: unlocked swap, a lost rollout only skips preview frames
            if item is None:
                time.sleep(.01); continue
            rows, obs, update = item
            for row in rows:
                if self.queue is not None:
                    break
                self.step(row, obs, update); time.sleep(1 / 30)

    def close(self):
        self.socket.close()


class Monitor:
    def __init__(self, out):
        self.out = out.resolve()
        self.lock = threading.Lock()
        self.live = None
        self.history = deque(maxlen=360)
        self.last_update = None
        self.training_control = TrainingControl(self.out)
        self.control_token = secrets.token_urlsafe(32)

    def replay(self):
        path = self.out / 'candidate.net.live'
        if path.stat().st_size > 96_000_000:
            raise ValueError('Replay exceeds monitor size limit')
        data = path.read_bytes()
        if len(data) < 16:
            raise ValueError('Incomplete evaluation recording')
        update, trials, steps, parts = struct.unpack_from('<4I', data)
        if not (1 <= trials <= 64 and 2 <= steps <= 36000 and parts == 21):
            raise ValueError('Invalid evaluation dimensions')
        if len(data) != 16 + 4 * trials + trials * steps * parts * 28:
            raise ValueError('Incomplete evaluation recording')
        return data

    def rig(self):
        rows = [line.split() for line in (self.out.parent / 'homer-build.txt').read_text().splitlines()
                if line.startswith('Char_')]
        calibration = json.loads((self.out / 'collider-frames.json').read_text())['body_to_collider_quaternions']
        if len(rows) != 21 or len(calibration) != 21:
            raise ValueError('Invalid native body geometry')
        parts = []
        for row, rotation in zip(rows, calibration):
            radius, half = map(float, row[3:5])
            box = None
            if half == 0:
                index = row.index('b')
                box = list(map(float, row[index + 1:index + 4]))
            numbers = [radius, half] + rotation + (box or [])
            if radius <= 0 or half < 0 or len(rotation) != 4 or not all(math.isfinite(x) for x in numbers):
                raise ValueError('Invalid native collider geometry')
            if abs(sum(x*x for x in rotation) - 1) > .02 or (box is not None and (len(box) != 3 or min(box) <= 0)):
                raise ValueError('Invalid native collider orientation or box')
            parts.append(dict(name=row[0], radius=radius, half=half, box=box, rotation=rotation))
        return parts

    def accept(self, data):
        value = json.loads(data)
        if not isinstance(value, dict) or value.get('run') != str(self.out):
            raise ValueError('Telemetry belongs to a different run')
        if value.get('phase') not in ('collecting', 'learning', 'evaluating', 'completed', 'failed'):
            raise ValueError('Unknown trainer phase')
        frame = value.get('frame')
        if frame is not None:
            points = frame['points']
            if len(points) != 21 or any(len(p) != 3 for p in points):
                raise ValueError('Invalid figure')
            numbers = [x for p in points for x in p] + [frame[k] for k in
                       ('speed', 'acceleration', 'trial_seconds', 'resets', 'captured_at')]
            if not all(isinstance(x, (int, float)) and math.isfinite(x) for x in numbers):
                raise ValueError('Invalid frame values')
        if not all(isinstance(value.get(k), (int, float)) and math.isfinite(value[k])
                   for k in ('update', 'sequence', 'sent_at')):
            raise ValueError('Invalid telemetry counters')
        with self.lock:
            self.live = value

    def snapshot(self):
        # Read complete atomic status files; a transient Windows reader lock leaves the next poll to retry.
        result = {}
        for name in ('status', 'progress', 'best'):
            try:
                result[name] = json.loads((self.out / (name + '.json')).read_text(encoding='utf-8'))
            except (OSError, ValueError):
                result[name] = {}
        with self.lock:
            result['live'] = self.live
            progress = result['progress']
            if progress.get('update') is not None and progress['update'] != self.last_update:
                self.history.append(dict(update=progress['update'], **progress.get('evaluation', {})))
                self.last_update = progress['update']
            result['history'] = list(self.history)
        result.update(server_time=time.time(), run=self.out.name, parents=PARENTS)
        result['training_control'] = self.training_control.snapshot()
        result['control_token'] = self.control_token
        try:
            tracking = json.loads((self.out / 'candidate.net.tracking.json').read_text())
            path = tracking.get('path')
            if (tracking['update'] == result['progress'].get('update') and isinstance(path, list)
                    and len(path) == 8 and all(isinstance(row, list) and len(row) == 3
                    and all(isinstance(x, (int, float)) and math.isfinite(x) for x in row) for row in path)):
                result['evaluation_path'] = dict(update=tracking['update'], trials=path)
        except (OSError, ValueError, KeyError):
            pass
        try:
            with (self.out / 'candidate.net.live').open('rb') as replay:
                update, trials, steps, parts = struct.unpack('<4I', replay.read(16))
            result['replay'] = dict(update=update, trials=trials, steps=steps, parts=parts)
        except (OSError, ValueError, struct.error):
            result['replay'] = None
        return result


def receiver(monitor, sock):
    while True:
        try:
            data, source = sock.recvfrom(16384)
            if source[0] == '127.0.0.1':
                monitor.accept(data)
        except (ValueError, TypeError, KeyError, UnicodeError):
            continue
        except OSError:
            return


def handler_for(monitor):
    class Handler(BaseHTTPRequestHandler):
        def allowed_host(self,host):
            defaults=(f'127.0.0.1:{self.server.server_port}',f'localhost:{self.server.server_port}')
            return host in getattr(self.server,'allowed_hosts',defaults)
        def do_POST(self):
            host = self.headers.get('Host', '')
            if (not self.allowed_host(host)
                    or self.headers.get('Origin', 'http://' + host) != 'http://' + host
                    or not secrets.compare_digest(self.headers.get('X-Delirium-Control', ''), monitor.control_token)):
                self.send_error(403); return
            if self.path not in ('/api/training/pause', '/api/training/resume', '/api/training/extend'):
                self.send_error(404); return
            try:
                length = int(self.headers.get('Content-Length', '0'))
                if not 1 <= length <= 128 or self.headers.get('Content-Type', '').split(';')[0] != 'application/json':
                    raise ValueError('Expected a small JSON request')
                body = json.loads(self.rfile.read(length))
                if self.path.endswith('/extend'):
                    if not isinstance(body, dict) or set(body) != {'target_updates'}:
                        raise ValueError('Expected only target_updates')
                    result = monitor.training_control.extend(body['target_updates'])
                else:
                    if body != {}:
                        raise ValueError('No request parameters are accepted')
                    action = monitor.training_control.pause if self.path.endswith('/pause') else monitor.training_control.resume
                    result = action()
                code = 200
            except (ValueError, OSError) as error:
                result, code = dict(error=str(error)), 409
            data = json.dumps(result).encode()
            self.send_response(code)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if not self.allowed_host(self.headers.get('Host','')):
                self.send_error(403); return
            if self.path == '/':
                data = Path(__file__).with_suffix('.html').read_bytes()
                kind = 'text/html; charset=utf-8'
            elif self.path == '/api/state':
                data = json.dumps(monitor.snapshot(), allow_nan=False).encode()
                kind = 'application/json'
            elif self.path == '/api/replay':
                try:
                    data = monitor.replay()
                except (OSError, ValueError):
                    self.send_error(503, 'Evaluation recording not ready'); return
                kind = 'application/octet-stream'
            elif self.path == '/api/rig':
                try:
                    data = json.dumps(monitor.rig(), allow_nan=False).encode()
                except (OSError, ValueError):
                    self.send_error(503, 'Native body geometry not ready'); return
                kind = 'application/json'
            else:
                self.send_error(404); return
            self.send_response(200)
            self.send_header('Content-Type', kind)
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Cache-Control', 'no-store')
            self.send_header('X-Content-Type-Options', 'nosniff')
            self.send_header('Content-Security-Policy', "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'")
            self.end_headers()
            try:
                self.wfile.write(data)
            except (ConnectionError, OSError):
                pass

        def log_message(self, *_):
            pass
    return Handler


def self_test():
    import tempfile
    import urllib.request
    row = [0.] * 301
    row[0] = 1.; row[4:7] = [0., 0., 0.]; row[13:16] = [.1, .2, -.3]
    row[193] = .2
    points, speed = body_frame(row)
    assert points[0] == [0., 0., 1.] and points[1] == [.1, .2, .7] and abs(speed - 1) < 1e-6
    udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); udp.bind(('127.0.0.1', 0)); udp.settimeout(1)
    out = Path(tempfile.mkdtemp(prefix='delirium-live-test-'))
    preview = LivePreview(udp.getsockname()[1], out); monitor = Monitor(out)
    replay = struct.pack('<4If', 17, 1, 2, 21, .5) + struct.pack('<294f', *([0.] * 294))
    (out / 'candidate.net.live').write_bytes(replay)
    assert monitor.replay() == replay and monitor.snapshot()['replay']['update'] == 17
    (out / 'candidate.net.live').write_bytes(replay[:-1])
    try:
        monitor.replay()
    except ValueError:
        pass
    else:
        raise AssertionError('Truncated evaluation recording accepted')
    (out / 'candidate.net.live').write_bytes(replay)
    preview.step(row, 293, 7); packet = udp.recv(16384); monitor.accept(packet)
    assert monitor.snapshot()['live']['frame']['speed'] == 1.
    row[294] = 1.; preview.last_sent = 0.; preview.step(row, 293, 8)
    monitor.accept(udp.recv(16384)); assert monitor.snapshot()['live']['frame']['resets'] == 1
    assert monitor.snapshot()['live']['frame']['trial_seconds'] == 0.
    sequence = preview.sequence
    preview.send('evaluating', 8, force=True); monitor.accept(udp.recv(16384))
    assert monitor.snapshot()['live']['phase'] == 'evaluating' and preview.sequence == sequence
    class Offline:
        def sendto(self, *_):
            raise OSError('Monitor closed')
    real_socket = preview.socket; preview.socket = Offline()
    preview.send('collecting', 9, force=True); assert preview.dropped == 1
    preview.socket = real_socket
    for broken in [b'{}', b'{', json.dumps(dict(run=str(out), phase='bad')).encode()]:
        try:
            monitor.accept(broken)
        except (ValueError, KeyError, TypeError):
            pass
        else:
            raise AssertionError('Invalid telemetry accepted')
    server = ThreadingHTTPServer(('127.0.0.1', 0), handler_for(monitor))
    worker = threading.Thread(target=server.serve_forever, daemon=True); worker.start()
    url = f'http://127.0.0.1:{server.server_port}'
    with urllib.request.urlopen(url + '/api/state') as response:
        assert response.headers['Cache-Control'] == 'no-store'
        assert json.load(response)['live']['update'] == 8
    with urllib.request.urlopen(url) as response:
        assert b'Training playback' in response.read()
    with urllib.request.urlopen(url + '/api/replay') as response:
        assert response.read() == replay
    server.shutdown(); server.server_close(); worker.join(); preview.close(); udp.close()
    print('Live decoding, UDP, resets, evaluation labels, offline monitor, validation and browser endpoints passed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--port', type=int, default=8765)
    parser.add_argument('--preview-port', type=int, default=8766)
    parser.add_argument('--tailscale-ip',help='Also bind HTTP to this explicit100.64.0.0/10 TailscaleIPv4 address; retain localhost')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
    else:
        assert args.out and args.out.is_dir(), 'Provide the existing training output directory'
        assert 1 <= args.port <= 65535 and 1 <= args.preview_port <= 65535
        monitor = Monitor(args.out)
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(('127.0.0.1', args.preview_port))
        server = ThreadingHTTPServer(('127.0.0.1', args.port), handler_for(monitor))
        remote=None
        if args.tailscale_ip:
            import ipaddress
            address=ipaddress.ip_address(args.tailscale_ip)
            if address not in ipaddress.ip_network('100.64.0.0/10'):
                server.server_close();sock.close();raise ValueError('Use thisPCs explicit TailscaleIPv4 address')
            try:remote=ThreadingHTTPServer((str(address),args.port),handler_for(monitor))
            except OSError:
                server.server_close();sock.close();raise
            hosts=(f'127.0.0.1:{args.port}',f'localhost:{args.port}',f'{address}:{args.port}')
            server.allowed_hosts=remote.allowed_hosts=hosts
            threading.Thread(target=remote.serve_forever,daemon=True).start()
            print(f'Tailscale viewer: http://{address}:{args.port}',flush=True)
        threading.Thread(target=receiver, args=(monitor, sock), daemon=True).start()
        print(f'Live training: http://127.0.0.1:{args.port} / {args.out.name}', flush=True)
        try:
            server.serve_forever()
        finally:
            if remote:remote.shutdown();remote.server_close()
            sock.close(); server.server_close()
