import subprocess
import os
import os.path
import time
import urllib.parse
import requests
import toml
import shutil
import pytest


class StateError(Exception):
    pass


class Server():
    _base_cfg_path = 'test_data/base_cfg.toml'

    @classmethod
    def base_config(cls):
        with open(cls._base_cfg_path, 'r') as f:
            return toml.load(f)

    def __init__(self, tmp_path, port=1337):
        self._port = port
        self._tmp_path = tmp_path
        self._cargo_args = '-F dev_no_pi,dev_no_motors'
        ret = subprocess.run(f'cargo build {self._cargo_args}'.split(),
                             capture_output=True,
                             stdin=subprocess.DEVNULL)
        try:
            ret.check_returncode()
        except subprocess.CalledProcessError as e:
            print('-'*10 + ' Error while compiling: ' + '-'*10)
            print('-'*10 + ' stderr: ' + '-'*10)
            print(e.stderr)
            print('-'*10 + ' stdout: ' + '-'*10)
            print(e.stderr)
            print('')
            raise e
        # we will set this in _start
        # using this we can ensure that restart doesn't get called if proc not
        # running etc.
        self._proc = None
        # we will set this to something once start gets called
        # using this we can ensure that start only gets called once
        self._cfg_path = None

    def _start(self):
        self._proc = subprocess.Popen(f'cargo run -q {self._cargo_args} -- -c {self._cfg_path} -p {self._port} -l trace'.split(),
                                     stdin=subprocess.DEVNULL)
        # wait for server to start, shouldn't take too long
        time.sleep(.2)

    def start(self, cfg=None, cfg_path=None):
        if self._proc != None or self._cfg_path != None:
            raise StateError()
        if cfg == None:
            if cfg_path == None:
                cfg = Server.base_config()
            else:
                with open(cfg_path, 'r') as f:
                    cfg = toml.load(f)
        # ensure that settings file is in the temporary directory so tests
        # dont influence each other
        cfg['general']['settings_path'] = os.path.join(self._tmp_path, 'settings.json')
        self._cfg_path = os.path.join(self._tmp_path, 'cfg.toml')
        with open(self._cfg_path, 'w') as f:
            toml.dump(cfg, f)
        self._start()

    def _ensure_started(self):
        if self._proc == None:
            raise StateError()

    def restart(self):
        self._ensure_started()
        self._stop()
        self._start()

    def _url(self, path):
        return urllib.parse.urljoin(f'http://localhost:{self._port}', path)

    def get(self, path, *args, **kwargs):
        self._ensure_started()
        return requests.get(self._url(path), *args, **kwargs)

    def post(self, path, *args, **kwargs):
        self._ensure_started()
        return requests.post(self._url(path), *args, **kwargs)

    def put(self, path, *args, **kwargs):
        self._ensure_started()
        return requests.put(self._url(path), *args, **kwargs)

    def _stop(self):
        if self._proc != None:
            self._proc.terminate()
            try:
                self._proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._proc.kill()
            assert self._proc.returncode == 0


@pytest.fixture()
def server(tmp_path):
    server = Server(tmp_path)
    yield server
    server._stop()


@pytest.fixture()
def prep_file(tmp_path):
    def _prep_file(path, perms=None):
        path = shutil.copy(path, tmp_path)
        if perms != None:
            os.chmod(path, perms)
        return path
    return _prep_file
