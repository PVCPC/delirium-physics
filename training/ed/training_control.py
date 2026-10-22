"""Checkpoint-and-exit pause, exact-command resume, and a duplicate-run lock."""
import ctypes
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

CAPABILITY = 'checkpoint-pause-v1'
LIMIT_CAPABILITY = 'extend-updates-v1'


def update_target(out, current):
    """Only raise the ceiling; malformed or older requests never shorten a run."""
    value = read_json(Path(out) / 'update-limit.json').get('target_updates')
    return max(current, value) if type(value) is int and 0 < value <= 2_000_000_000 else current


def update_range(out, first, target):
    update = first
    while True:
        target = update_target(out, target)
        if update >= target:
            return
        yield update, target
        update += 1


def read_json(path):
    try:
        return json.loads(path.read_text(encoding='utf-8'))
    except (OSError, ValueError):
        return {}


def write_json(path, value):
    temporary = path.with_suffix('.control-tmp')
    temporary.write_text(json.dumps(value, indent=2), encoding='utf-8')
    for attempt in range(20):
        try:
            os.replace(temporary, path)
            return
        except PermissionError:
            if attempt == 19:
                raise
            time.sleep(.01)


class RunLock:
    def __init__(self, out):
        self.path = Path(out) / 'trainer.lock'
        self.file = None

    def acquire(self):
        f = self.path.open('a+b')
        if not f.tell():
            f.write(b'0'); f.flush()
        f.seek(0)
        try:
            if os.name == 'nt':
                import msvcrt
                msvcrt.locking(f.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except (OSError, BlockingIOError):
            f.close()
            return False
        self.file = f
        return True

    def release(self):
        if self.file is not None:
            self.file.seek(0)
            if os.name == 'nt':
                import msvcrt
                msvcrt.locking(self.file.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.file.fileno(), fcntl.LOCK_UN)
            self.file.close(); self.file = None


def locked(out):
    lock = RunLock(out)
    if not lock.acquire():
        return True
    lock.release()
    return False


def alive(pid):
    if not isinstance(pid, int) or pid <= 0:
        return False
    if os.name == 'nt':
        kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel.OpenProcess.restype = ctypes.c_void_p
        handle = kernel.OpenProcess(0x1000, False, pid)
        if not handle:
            return False
        code = ctypes.c_ulong()
        try:
            return bool(kernel.GetExitCodeProcess(ctypes.c_void_p(handle), ctypes.byref(code))) and code.value == 259
        finally:
            kernel.CloseHandle(ctypes.c_void_p(handle))
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


class TrainerSession:
    def __init__(self, out):
        self.out = Path(out).resolve()
        self.lock = RunLock(self.out)

    def __enter__(self):
        self.out.mkdir(parents=True, exist_ok=True)
        if not self.lock.acquire():
            raise RuntimeError('This output already has a running trainer')
        try:
            write_json(self.out / 'trainer-launch.json', dict(executable=sys.executable,
                script=str(Path(__file__).with_name('train_motion.py').resolve()), arguments=sys.argv[1:],
                cwd=os.getcwd(), out=str(self.out), pid=os.getpid(), capability=CAPABILITY))
            write_json(self.out / 'training-request.json', dict(command='running', requested_at=time.time()))
        except BaseException:
            self.lock.release()
            raise
        return self

    def __exit__(self, *_):
        self.lock.release()


class PauseRequest:
    def __init__(self, out):
        self.path = Path(out) / 'training-request.json'
        self.next_check = 0.
        self.paused = False

    def requested(self):
        if time.monotonic() >= self.next_check:
            self.paused = read_json(self.path).get('command') == 'pause'
            self.next_check = time.monotonic() + .25
        return self.paused


class TrainingControl:
    def __init__(self, out):
        self.out = Path(out).resolve()
        self.mutex = threading.Lock()
        self.pending = None

    def snapshot(self):
        status = read_json(self.out / 'status.json')
        supported = status.get('training_control') == CAPABILITY
        running = status.get('status') == 'running' and alive(status.get('pid'))
        busy = locked(self.out)
        request = read_json(self.out / 'training-request.json')
        requested = request.get('command') == 'pause'
        launch_pid = read_json(self.out / 'trainer-launch.json').get('pid')
        starting = ((request.get('command') == 'starting' and alive(request.get('pid'))) or
                    (launch_pid != status.get('pid') and alive(launch_pid)))
        pending = starting or (self.pending is not None and self.pending.poll() is None)
        phase = 'starting' if pending and not running else ('pausing' if requested and running else status.get('status', 'unavailable'))
        progress = read_json(self.out / 'progress.json')
        target = update_target(self.out, status.get('target_updates', 0))
        finished_at_limit = status.get('status') == 'completed' and status.get('stop_reason') == 'update_limit'
        extendable = supported and status.get('update_limit_control') == LIMIT_CAPABILITY and (
                    (running and not requested) or status.get('status') == 'paused' or (finished_at_limit and not busy and not alive(status.get('pid'))))
        handed_off = (self.out / 'continuation-result.json').exists() or read_json(self.out / 'completion-hook/receipt.json').get('phase') == 'delivery_uncertain'
        if finished_at_limit and handed_off:
            extendable = False
        return dict(phase=phase, pause_enabled=supported and running and not requested,
                    resume_enabled=supported and (status.get('status') == 'paused' or
                        (finished_at_limit and not handed_off and target > status.get('target_updates', target)))
                        and not alive(status.get('pid')) and not busy and not pending,
                    checkpoint_update=progress.get('update'), target_updates=target,
                    applied_target_updates=status.get('target_updates'), extend_enabled=extendable and not pending,
                    extension_pending=running and target > status.get('target_updates', 0),
                    stop_reason=status.get('stop_reason'))

    def pause(self):
        with self.mutex:
            state = self.snapshot()
            if state['phase'] in ('paused', 'pausing'):
                return state
            if not state['pause_enabled']:
                raise ValueError('Training is not running with checkpoint pause enabled')
            write_json(self.out / 'training-request.json', dict(command='pause', requested_at=time.time()))
            return self.snapshot()

    def resume(self):
        with self.mutex:
            if self.pending is not None and self.pending.poll() is None:
                return self.snapshot()
            state = self.snapshot()
            if not state['resume_enabled']:
                raise ValueError('Resume requires a saved pause or an update-limit run with a saved higher limit')
            return self._start()

    def extend(self, target_updates):
        with self.mutex:
            state = self.snapshot()
            if type(target_updates) is not int or not 0 < target_updates <= 2_000_000_000:
                raise ValueError('Enter a whole-number total update target between 1 and 2,000,000,000')
            if not state['extend_enabled']:
                raise ValueError('Extend a running, paused, or update-limit-finished run; plateau stops require review')
            if target_updates <= max(state['target_updates'], state['checkpoint_update'] or 0):
                raise ValueError('The new total must be greater than the current target and saved update')
            launch = self._saved_launch()
            write_json(self.out / 'update-limit.json', dict(target_updates=target_updates, requested_at=time.time(),
                       previous_target=state['target_updates'], requested_by='viewer'))
            # Keep exact recovery commands in sync; the running trainer also polls the durable request.
            arguments = list(launch['arguments'])
            arguments[arguments.index('--updates') + 1] = str(target_updates)
            launch['arguments'] = arguments
            write_json(self.out / 'trainer-launch.json', launch)
            if state['phase'] == 'completed':
                return self._start()
            return self.snapshot()

    def _saved_launch(self):
        launch = read_json(self.out / 'trainer-launch.json')
        arguments = launch.get('arguments', [])
        script = Path(__file__).with_name('train_motion.py').resolve()
        try:
            valid = (launch.get('capability') == CAPABILITY and Path(launch.get('out', '')).resolve() == self.out
                and Path(launch.get('script', '')).resolve() == script and '--rust-learner' in arguments
                and Path(arguments[arguments.index('--out') + 1]).resolve() == self.out
                  and '--updates' in arguments and arguments.index('--updates') + 1 < len(arguments)
                  and Path(launch.get('executable', '')).is_file() and (self.out / 'ppo.pt').is_file())
        except (ValueError, IndexError, TypeError):
            valid = False
        if not valid:
            raise ValueError('Saved trainer launch does not match this run')
        return launch

    def _start(self):
        launch = self._saved_launch()
        arguments = list(launch['arguments'])
        old_target = int(arguments[arguments.index('--updates') + 1])
        arguments[arguments.index('--updates') + 1] = str(update_target(self.out, old_target))
        with (self.out / 'training-controls.log').open('ab') as log, (self.out / 'training-controls-errors.log').open('ab') as errors:
            self.pending = subprocess.Popen([launch['executable'], launch['script'], *arguments], cwd=launch['cwd'],
                stdout=log, stderr=errors, creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
        write_json(self.out / 'training-request.json', dict(command='starting', pid=self.pending.pid, requested_at=time.time()))
        self._follow_completion()
        return dict(self.snapshot(), phase='starting', pause_enabled=False, resume_enabled=False, extend_enabled=False)

    def _follow_completion(self):
        """An explicit extension starts a new exit cycle; keep earlier delivery evidence."""
        folder = self.out / 'completion-hook'
        config = read_json(folder / 'config.json')
        if not config.get('enabled'):
            return
        try:
            receipt = read_json(folder / 'receipt.json')
            if receipt.get('phase') == 'queued':
                deadline = time.monotonic() + 2
                while locked(folder) and time.monotonic() < deadline:
                    time.sleep(.05)
                if locked(folder):
                    raise ValueError('Previous completion watcher is still closing')
                write_json(folder / f'receipt-before-extension-{time.time_ns()}.json', receipt)
                write_json(folder / 'receipt.json', dict(phase='user_extended', at=time.time()))
            from training_completion import arm
            arm(self.out, config['thread'], Path(config['codex']))
        except (OSError, ValueError) as error:
            # Optional exit notification must never kill healthy training.
            write_json(folder / 'extension-error.json', dict(error=str(error), at=time.time()))
