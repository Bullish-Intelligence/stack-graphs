from typing import ClassVar, Generic, TypeVar, TypedDict
from typing_extensions import dataclass_transform
from pydantic import BaseModel, PrivateAttr, Field
import pydantic
from dataclasses import dataclass
from attrs import define

T = TypeVar("T")

class M(BaseModel):
    name: str
    size: int = 0
    _secret: int = 0
    kind: ClassVar[str] = "m"
    cache: int = PrivateAttr()
    model_config = {}

M(name="a", size=1)
# ^ defined: 11
#           ^ defined: 12
M(_secret=1, kind="x", cache=2, model_config=3, other=4)
# ^ defined:
#            ^ defined:
#                      ^ defined:
#                               ^ defined:
#                                               ^ defined:

# A model class derives from a model class: the fields add up.
class N(M):
    extra: str = ""

N(name="a", extra="b")
# ^ defined: 11
#           ^ defined: 30

# A model class derives from a model class two bases away.
class O(N):
    deep: bool = False

O(name="a", extra="b", deep=True)
# ^ defined: 11
#           ^ defined: 30
#                      ^ defined: 38

# A module attribute as the base, a second base, and a generic base.
class P(pydantic.BaseModel, Generic[T]):
    value: T

P(value=1)
# ^ defined: 47

class Q(P[int]):
    other: int = 0

Q(value=1, other=2)
# ^ defined: 47
#          ^ defined: 53

# A generic class is called with a subscript.
P[int](value=1)
#      ^ defined: 47

# A class that derives from a class that is no model class is no model class.
class Plain:
    unrelated: int = 0

class R(Plain):
    own: int = 0

R(own=1)
# ^ defined:

# A written `__init__` wins.
class S(BaseModel):
    a: int = 0

    def __init__(self, b):
        pass

S(b=1)
# ^ defined: 77
S(a=1)
# ^ defined:

# A class marked with `dataclass_transform` makes its subclasses model classes.
@dataclass_transform()
class ModelBase:
    pass

class U(ModelBase):
    field_u: int = 0

class V(U):
    field_v: int = 0

V(field_u=1, field_v=2)
# ^ defined: 91
#            ^ defined: 94
U(field_u=1)
# ^ defined: 91
ModelBase(field_u=1)
#         ^ defined:

# A TypedDict takes all fields, with total=False, and inherits fields.
class TD(TypedDict, total=False):
    a: int
    _b: str

class TD2(TD):
    c: int

TD(a=1, _b="x")
#  ^ defined: 106
#       ^ defined: 107
TD2(a=1, c=2, _b="y")
#   ^ defined: 106
#        ^ defined: 110
#             ^ defined: 107


# A generic dataclass, with a base class of the project and a subscript call.
@dataclass
class G(Generic[T]):
    g1: T

@dataclass
class H(G[int]):
    h1: int = 0

H(g1=1, h1=2)
# ^ defined: 124
#       ^ defined: 128
H[int](g1=1)
#      ^ defined: 124
G[int](g1=1)
#      ^ defined: 124

# Decorators of other libraries that `dataclass_transform` marks.
@pydantic.dataclasses.dataclass
class PD:
    pd1: int

@define
class AD:
    ad1: int

PD(pd1=1)
#  ^ defined: 141
AD(ad1=1)
#  ^ defined: 145

# The fields of a dataclass with init=False still reach a subclass.
@dataclass(init=False)
class NI:
    ni1: int

@dataclass
class NJ(NI):
    nj1: int = 0

NJ(ni1=1, nj1=2)
#  ^ defined: 155
#         ^ defined: 159
