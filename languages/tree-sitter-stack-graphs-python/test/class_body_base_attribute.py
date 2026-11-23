from typing import Tuple


class Request:
    attributes: Tuple[str, ...] = ("url",)


class JsonRequest(Request):

    attributes: Tuple[str, ...] = Request.attributes + ("dumps_kwargs",)
#                                 ^ defined: 4
#                                         ^ defined: 5


class Json2(Request):
    attributes = Request.attributes + ("dumps_kwargs",)
#                ^ defined: 4
#                        ^ defined: 5


class Json3(Request):
    attributes: Tuple[str, ...]
    other = Request.attributes
#           ^ defined: 4
#                   ^ defined: 5
