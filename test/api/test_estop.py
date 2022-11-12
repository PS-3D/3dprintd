def test_estop(server):
    server.start()
    r = server.post('/v0/estop')
    assert r.status_code == 202
