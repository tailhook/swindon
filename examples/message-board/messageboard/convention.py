import base64
import json
import logging
from functools import wraps

from sanic.response import json as response

log = logging.getLogger(__name__)

class User(object):

    def __init__(self, user_id):
        self.user_id = user_id
        # this is a hack to get rid of DB
        self.username = user_id.replace('_', ' ').title()

    def __repr__(self):
        return "<User {}>".format(self.user_id)


class Connection(object):

    def __init__(self, connection_id):
        self.connection_id = connection_id

    def __repr__(self):
        return "<Connection {}>".format(self.connection_id)


class Request(object):

    def __init__(self, auth, *, request_id, connection_id, **_unused):
        self.request_id = request_id
        self.connection = Connection(connection_id)
        kind, value = auth.split(' ')
        assert kind == 'Tangle'
        auth = json.loads(
            base64.b64decode(value.encode('ascii')).decode('utf-8'))
        self.user = User(**auth)

    def __repr__(self):
        return "<Request of {!r}>".format(self.user)



def swindon_convention(f):
    @wraps(f)
    async def swindon_call_method(request):
        req = None
        try:
            metadata, args, kwargs = request.json
            req = Request(request.headers.get("Authorization"), **metadata)
            result = await f(req, *args, **kwargs)
            return response(result)
        except Exception as e:
            log.exception("Error for %r", req or request, exc_info=e)
            raise
    return swindon_call_method

