"""Pelican RLS2 codec and a bounded, ordered client for rlearn serve."""
from concurrent.futures import ThreadPoolExecutor, TimeoutError
import os
from pathlib import Path
import struct
import subprocess

import numpy as np

HEADER = struct.Struct('<4s4IiQQI')


def count(dimensions):
    o, h, a = dimensions
    if not all(isinstance(x, int) and 0 < x <= 65536 for x in dimensions):
        raise ValueError('Invalid learner architecture')
    n = 2 * (o * h + h + h * h + h) + h * a + 2 * a + h + 1
    if n > 4_000_000:
        raise ValueError('Learner state too large')
    return n


def encode(state):
    dims = tuple(state['dimensions']); n = count(dims)
    previous = state.get('snapshot')
    step, updates, rng = state['adam_step'], state['completed_updates'], state.get('rng', 18)
    if not isinstance(step, int) or not 0 <= step <= 2**31 - 1 or not 0 <= updates < 2**64 or not 0 < rng < 2**64:
        raise ValueError('Invalid state counters or RNG')
    chunks = []
    for name in ('parameters', 'exp_avg', 'exp_avg_sq'):
        value = np.asarray(state[name])
        if value.dtype != np.float32 or value.shape != (n,) or not np.isfinite(value).all():
            raise ValueError('Invalid state vector: ' + name)
        if name == 'exp_avg_sq' and np.any(value < 0):
            raise ValueError('Negative second moment')
        chunks.append(value.astype('<f4', copy=False).tobytes())
    if previous is not None:
        previous = np.asarray(previous)
        if previous.dtype != np.float32 or previous.shape != (n,) or not np.isfinite(previous).all():
            raise ValueError('Invalid previous policy snapshot')
        chunks.append(previous.astype('<f4', copy=False).tobytes())
    return HEADER.pack(b'RLS2', *dims, n, step, rng, updates, int(previous is not None)) + b''.join(chunks)


def decode(blob):
    if len(blob) < HEADER.size:
        raise ValueError('Truncated RLS2 header')
    magic, o, h, a, n, step, rng, updates, has = HEADER.unpack_from(blob)
    if magic != b'RLS2' or has not in (0, 1) or count((o, h, a)) != n:
        raise ValueError('Invalid RLS2 header')
    if len(blob) != HEADER.size + 4 * n * (3 + has):
        raise ValueError('RLS2 length mismatch')
    values = np.frombuffer(blob, dtype='<f4', offset=HEADER.size).copy()
    state = dict(dimensions=(o, h, a), adam_step=step, rng=rng, completed_updates=updates,
                 parameters=values[:n], exp_avg=values[n:2*n], exp_avg_sq=values[2*n:3*n],
                 snapshot=values[3*n:] if has else None)
    encode(state)  # Apply the same finite/counter/buffer checks to loaded files.
    return state


def read_exact(stream, size):
    chunks = []; remaining = size
    while remaining:
        data = stream.read(remaining)
        if not data:
            raise EOFError('Rust learner closed its output before replying')
        chunks.append(data); remaining -= len(data)
    return b''.join(chunks)


class Client:
    def __init__(self, exe, dimensions, envs, horizon, threads, initial, folder, learning_rate=1e-4):
        self.dimensions = tuple(dimensions); self.n = count(self.dimensions)
        if not 1 <= envs <= 4096 or not 1 <= horizon <= 4096 or not 1 <= threads <= 64:
            raise ValueError('Invalid worker/batch configuration')
        self.envs, self.horizon = envs, horizon
        self.folder = Path(folder).resolve(); self.folder.mkdir(exist_ok=True, parents=True)
        state = decode(initial)
        if state['dimensions'] != self.dimensions:
            raise ValueError('Initial state architecture differs')
        self.pending = False
        self.reader = ThreadPoolExecutor(max_workers=1)
        if not np.isfinite(learning_rate) or not 0 < learning_rate <= 1e-4:
            raise ValueError('Invalid learner learning rate')
        command = [str(exe), 'serve'] if learning_rate == 1e-4 else [str(exe), 'serve-lr', str(learning_rate)]
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        try:
            self.process.stdin.write(struct.pack('<6I', *self.dimensions, envs, horizon, threads))
            self.process.stdin.write(state['parameters'].astype('<f4', copy=False).tobytes())
            self.process.stdin.flush()
            self.load(initial)
        except BaseException:
            self.close()
            raise

    def read(self, size):
        future = self.reader.submit(read_exact, self.process.stdout, size)
        try:
            return future.result(timeout=60)
        except TimeoutError:
            self.process.terminate()
            raise RuntimeError('Rust learner did not reply within 60 seconds') from None

    def command(self, tag, path):
        if self.pending:
            raise RuntimeError('Read the pending batch before saving/loading learner state')
        name = str(path).encode('utf-8')
        if len(name) > 32768:
            raise ValueError('State path too long')
        self.process.stdin.write(struct.pack('<II', tag, len(name)) + name); self.process.stdin.flush()
        status, updates = struct.unpack('<IQ', self.read(12))
        if status:
            raise RuntimeError(f'Rust state command {tag} failed for {path}')
        return updates

    def load(self, blob):
        state = decode(blob)
        if state['dimensions'] != self.dimensions:
            raise ValueError('Loaded state architecture differs')
        path = self.folder / 'load.rls'; temporary = path.with_suffix('.tmp')
        temporary.write_bytes(blob); os.replace(temporary, path)
        updates = self.command(2, path)
        if updates != state['completed_updates']:
            raise RuntimeError('Rust load acknowledged the wrong update counter')
        return state

    def save(self):
        path = self.folder / 'snapshot.rls'
        updates = self.command(1, path)
        blob = path.read_bytes(); state = decode(blob)
        if updates != state['completed_updates']:
            raise RuntimeError('Rust save acknowledged the wrong update counter')
        return blob, state

    def submit(self, obs, actions, rewards, dones, last):
        if self.pending:
            raise RuntimeError('Only one learner batch may be in flight')
        o, _, a = self.dimensions; n = self.envs * self.horizon
        arrays = []
        for value, size in zip((obs, actions, rewards, dones, last), (n*o, n*a, n, n, self.envs*o)):
            value = np.asarray(value, dtype='<f4')
            if value.size != size or not np.isfinite(value).all():
                raise ValueError('Invalid learner batch')
            arrays.append(value.tobytes())
        self.process.stdin.write(struct.pack('<I', 0))
        for value in arrays:
            self.process.stdin.write(value)
        self.process.stdin.flush(); self.pending = True

    def receive(self):
        if not self.pending:
            raise RuntimeError('No learner reply is pending')
        data = np.frombuffer(self.read(4 * (self.n + 2)), dtype='<f4').copy()
        self.pending = False
        if not np.isfinite(data).all():
            raise ValueError('Non-finite learner response')
        return float(data[0]), float(data[1]) / 1000., data[2:]

    def close(self):
        if self.process.poll() is None:
            if self.pending:
                self.process.terminate()
            else:
                self.process.stdin.close()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.terminate(); self.process.wait(timeout=5)
        self.reader.shutdown(wait=True, cancel_futures=True)
        for pipe in (self.process.stdin, self.process.stdout):
            if pipe and not pipe.closed:
                pipe.close()
