#define _DMZMQ_SO "dmzmq/libdmzmq.so"
#include "dmzmq/_package.dm"

/mob/verb/params()
	usr << "params:"
	for(var/p in world.params)
		usr << "[p] = [world.params[p]]"

/proc/m(a, list/b)
	return list("", json_encode(list("[a]" = (b.len == 1? b[1]: b))))

/world/New()
	. = ..()
	world.log << json_encode(world.params)
	dmzmq_setup()
	var/datum/zmq_socket/deal_sock = new(ZMQ_DEALER)
	deal_sock.connect(world.params["supermatter_endpoint"])
	deal_sock.send_multi(m("ServerStarted", list(world.params["supermatter_id"])))
	deal_sock.send_multi(m("RunUpdate", list(world.params["supermatter_id"], list("FOO" = "hello, world!"))))

	var/pc = 0
	var/updatin = 0

	while(1)
		if(pc == 6 && !updatin)
			updatin = 1

		var/list/msg = deal_sock.recv_multi()
		world.log << json_encode(msg)
		if(msg[1] == "PING")
			pc++
			deal_sock.send_multi(m("Pong", list(world.params["supermatter_id"])))
