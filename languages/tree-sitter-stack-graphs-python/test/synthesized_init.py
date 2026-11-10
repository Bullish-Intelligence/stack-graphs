from dataclasses import dataclass, field
import dataclasses
from typing import ClassVar, NamedTuple

@dataclass
class P:
    x: int
    y: int = 0
    limit: ClassVar[int] = 3
    hidden: int = field(default=0, init=False)

    def m(self):
        pass

P(x=1, y=2)
# ^ defined: 7
#      ^ defined: 8
P(limit=1, hidden=2, m=3)
# ^ defined:
#          ^ defined:
#                    ^ defined:

# A subclass of a dataclass inherits the fields of the base class.
@dataclasses.dataclass(frozen=True)
class Q(P):
    z: str = ""

Q(x=1, z="a")
# ^ defined: 7
#      ^ defined: 26

# A plain subclass uses the synthesized `__init__` of its base class.
class R(Q):
    pass

R(z="b")
# ^ defined: 26

# A class that writes its own `__init__` gets no synthesized `__init__`.
@dataclass
class S:
    a: int

    def __init__(self, b):
        self.a = b

S(b=1)
# ^ defined: 44
S(a=1)
# ^ defined:

class T(NamedTuple):
    name: str
    size: int = 0

T(name="t", size=1)
# ^ defined: 53
#           ^ defined: 54

# A decorator that is not `dataclass` synthesizes nothing.
def other(cls):
    return cls

@other
class U:
    u: int

U(u=1)
# ^ defined:
