import libp2p/[standard_setup, peerinfo, multiaddress]
import chronos
import options

proc doit() {.async.} =
  let
    node = newStandardSwitch(gossip = true, privKey = some(PrivateKey.random(RSA).tryGet()))
    sfut = node.start()
    handlerFut = newFuture[void]()
    remote = PeerInfo.init(
      "QmT8DUL7pv43udotKuGbx2SCT48A3oJwqyXTmSygkvVNeB", 
      [MultiAddress.init("/ip4/127.0.0.1/tcp/54777")])

  proc handler(topic: string, data: seq[byte]) {.async.} =
    assert topic == "test-net"
    handlerFut.complete()

  await node.connect(remote)
  await node.subscribe("test-net", handler)
  await node.stop()
  discard await sfut
  echo "Done"

waitFor doit()