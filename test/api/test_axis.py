import pytest
import json


def test_position(server):
    server.start()
    r = server.get('/v0/axis/position')
    assert r.status_code == 200
    data = json.loads(r.text)
    assert isinstance(data['x']['position'], float)
    assert isinstance(data['y']['position'], float)
    assert isinstance(data['z']['position'], float)
    # doing it this way because it is not specified in what position the printer
    # has to start in
    expected = {'x': {'position': data['x']['position']},
                'y': {'position': data['y']['position']},
                'z': {'position': data['z']['position']},}
    assert data == expected


@pytest.mark.parametrize('axis', ['x', 'y', 'z'])
def test_axis_position(server, axis):
    server.start()
    r = server.get(f'/v0/axis/{axis}/position')
    assert r.status_code == 200
    data = json.loads(r.text)
    assert isinstance(data['position'], float)
    # doing it this way because it is not specified in what position the printer
    # has to start in
    expected = {'position': data['position']}
    assert data == expected


@pytest.mark.parametrize('axis', ['x', 'y', 'z'])
def test_axis_settings_cfg_defaults(server, axis):
    ref_speed = 1
    ref_acc_decc = 1
    ref_jerk = 1
    cfg = type(server).base_config()
    cfg['motors'][axis]['default_reference_speed'] = ref_speed
    cfg['motors'][axis]['default_reference_accel'] = ref_acc_decc
    cfg['motors'][axis]['default_reference_jerk'] = ref_jerk
    server.start(cfg=cfg)
    r = server.get(f'/v0/axis/{axis}/settings')
    assert r.status_code == 200
    data = json.loads(r.text)
    expected = {'reference_speed': ref_speed,
                'reference_accel_decel': ref_acc_decc,
                'reference_jerk': ref_jerk}
    assert data == expected


@pytest.mark.parametrize('axis', ['x', 'y', 'z'])
def test_axis_settings_put_single(server, axis):
    server.start()
    ref_speed = 1
    r = server.get(f'/v0/axis/{axis}/settings')
    assert r.status_code == 200
    old_data = json.loads(r.text)
    r = server.put(f'/v0/axis/{axis}/settings', json={'reference_speed': ref_speed})
    assert r.status_code == 200
    r = server.get(f'/v0/axis/{axis}/settings')
    assert r.status_code == 200
    data = json.loads(r.text)
    expected = {'reference_speed': ref_speed,
                'reference_accel_decel': old_data['reference_accel_decel'],
                'reference_jerk': old_data['reference_jerk']}
    assert data == expected
