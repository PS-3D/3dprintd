import os.path
import json


def test_gcode_stopped(server):
    server.start()
    r = server.get('/v0/gcode')
    assert r.status_code == 200
    data = json.loads(r.text)
    expected = {'status': 'stopped'}
    assert data == expected


def _start_benchy(server):
    path = 'test_data/gcode/benchy_first_layer.gcode'
    path = os.path.abspath(path)
    r = server.post('/v0/gcode/start', json={'path': path})
    assert r.status_code == 202
    r = server.get('/v0/gcode')
    assert r.status_code == 200
    data = json.loads(r.text)
    assert isinstance(data['line'], int)
    expected = {'status': 'printing', 'path': path, 'line': data['line']}
    assert data == expected
    return path


def test_gcode_start(server):
    server.start()
    _start_benchy(server)


def test_gcode_start_no_open(server, prep_file):
    server.start()
    path = prep_file('test_data/gcode/benchy_first_layer.gcode', perms=0)
    r = server.post('/v0/gcode/start', json={'path': path})
    assert r.status_code == 512


def test_gcode_start_already_printing(server, prep_file):
    server.start()
    path = _start_benchy(server)
    path2 = prep_file('test_data/gcode/benchy_first_layer.gcode', rename='benchy2.gcode')
    r = server.post('/v0/gcode/start', json={'path': path2})
    assert r.status_code == 409
    r = server.get('/v0/gcode')
    data = json.loads(r.text)
    # assuming the response is still mostly the same as before, so we only check
    # the path thats being printed
    assert data['path'] == path


def test_gcode_stop(server):
    server.start()
    _start_benchy(server)
    r = server.post('/v0/gcode/stop')
    assert r.status_code == 202
    r = server.get('/v0/gcode')
    assert r.status_code == 200
    data = json.loads(r.text)
    expected = {'status': 'stopped'}
    assert data == expected


def _start_benchy_pause(server):
    path = _start_benchy(server)
    r = server.post('/v0/gcode/pause')
    assert r.status_code == 202
    r = server.get('/v0/gcode')
    assert r.status_code == 200
    data = json.loads(r.text)
    assert isinstance(data['line'], int)
    expected = {'status': 'paused', 'path': path, 'line': data['line']}
    assert data == expected
    return path


def test_gcode_pause(server):
    server.start()
    _start_benchy_pause(server)


def test_benchy_pause_stopped(server):
    server.start()
    r = server.post('/v0/gcode/pause')
    assert r.status_code == 409


def test_gcode_continue(server):
    server.start()
    path = _start_benchy_pause(server)
    r = server.post('/v0/gcode/continue')
    assert r.status_code == 202
    r = server.get('/v0/gcode')
    assert r.status_code == 200
    data = json.loads(r.text)
    assert isinstance(data['line'], int)
    expected = {'status': 'printing', 'path': path, 'line': data['line']}
    assert data == expected


def test_benchy_continue_stopped(server):
    server.start()
    r = server.post('/v0/gcode/continue')
    assert r.status_code == 409
