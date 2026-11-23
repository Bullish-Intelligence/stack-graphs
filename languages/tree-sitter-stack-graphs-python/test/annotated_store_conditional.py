from typing import Any, Literal, Protocol


class Codec(Protocol):
    def dump(self, tp: Any, value: Any) -> Any: ...


class IdentityCodec(Codec):
    def dump(self, tp: Any, value: Any) -> Any:
        return value


class JsonCodec(Codec):
    def dump(self, tp: Any, value: Any) -> Any:
        return value


IDENTITY: Codec = IdentityCodec()
JSON: Codec = JsonCodec()


class Typed:
    def __init__(self, mode: Literal['json', 'identity'] = 'json') -> None:
        self._codec: Codec = JSON if mode == 'json' else IDENTITY
        self._plain: Any = IdentityCodec()

    def dump(self, value):
        return self._codec.dump(None, value)
#                          ^ defined: 5

    def dump_plain(self, value):
        return self._plain.dump(None, value)
#                          ^ defined: 9
