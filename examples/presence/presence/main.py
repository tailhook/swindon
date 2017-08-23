import os
import logging
import argparse
from aiohttp import web
from censusname import generate as make_name

from .convention import swindon_convention
from .swindon import connect


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--port', default=8082, help="Listen port")
    ap.add_argument('--swindon-port', default=8081,
        help="Connect to swindon at port")
    options = ap.parse_args()

    logging.basicConfig(level=logging.DEBUG)
    app = web.Application()
    app['swindon'] = connect(('localhost', options.swindon_port))
    app.router.add_route("POST", "/tangle/authorize_connection", auth)
    app.router.add_route("POST", "/message", message)


    if os.environ.get("LISTEN_FDS") == '1':
        # Systemd socket activation protocol
        import socket
        sock = socket.fromfd(3, socket.AF_INET, socket.SOCK_STREAM)
        web.run_app(app, sock=sock)
    else:
        web.run_app(app, port=options.port)


@swindon_convention
async def auth(req, http_authorization, http_cookie, url_querystring):
    name = make_name()
    id = name.lower().replace(' ', '_')
    await req.app['swindon'].subscribe(req.connection, 'message-board')
    return {
        'user_id': id,
        'username': name,
    }


@swindon_convention
async def message(req, text):
    await req.app['swindon'].publish('message-board', {
        'author': req.user.username,
        'text': text,
        })
    return True
