#define DEBUG

#define _DMZMQ_SO "dmzmq/libdmzmq.so"
#include "dmzmq/_package.dm"

/datum/zmq_socket/callback/supervisor_socket
	var/static/list/message_handlers = null

/datum/zmq_socket/callback/supervisor_socket/New(new_sock_type = ZMQ_DEALER)
	ASSERT(new_sock_type == ZMQ_DEALER)

	if(src.message_handlers == null)
		src.message_handlers = list()

		var/regex/re = new("^[REGEX_QUOTE("[src.type]")]/(proc/handle_(\[a-zA-Z0-9]+))$")

		for(var/p in typesof("[src.type]/proc"))
			world.log << p
			if(re.Find(p))
				world.log << p
				src.message_handlers[re.group[2]] = re.group[1]

		world.log << json_encode(src.message_handlers)

	return ..()

/datum/zmq_socket/callback/supervisor_socket/proc/send_msg()
	ASSERT(args.len >= 1)
	
	var/id = args[1]
	var/list/msg_args = args.Copy(2)

	// not ideal, too much list()
	src.send_multi(list("", json_encode(list("[id]" = (msg_args.len == 1? msg_args[1]: msg_args)))))

/datum/zmq_socket/callback/supervisor_socket/on_msg(list/data)
	ASSERT(data[1] == "")

	var/list/payload = json_decode(data[2])

	var/msg_id = payload[1]
	var/list/msg_args = payload[msg_id]

	// serde will serialize Message(String) as {"Message": String}, but Message(String, String) as {"Message": [String, String]}
	// this step normalizes it to always be a list regardless of arg count
	if(!istype(msg_args))
		msg_args = list(msg_args)
	
	world.log << src.message_handlers[msg_id]
	call(src, src.message_handlers[msg_id])(arglist(msg_args))

/datum/zmq_socket/callback/supervisor_socket/proc/handle_Ping()
	src.send_msg("Pong", world.params["supermatter_id"])

/proc/pollard()
	set waitfor = 0
	while(1)
		callback_socket_pollset.poll()
		sleep(1)

/world/New()
	. = ..()
	world.log << json_encode(world.params)
	dmzmq_setup()

	pollard()

	var/datum/zmq_socket/callback/supervisor_socket/suvi_sock = new
/*	suvi_sock.connect(world.params["supermatter_endpoint"])

	suvi_sock.send_msg("ServerStarted", world.params["supermatter_id"])
	suvi_sock.send_msg("RunUpdate", world.params["supermatter_id"], list("FOO" = "hello, world!"))*/
