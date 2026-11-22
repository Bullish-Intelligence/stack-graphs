# A return annotation that names a type variable or an alias keeps the value following of the
# body. The definition of the name decides. It does not matter which file defines the name, which
# name the annotation uses, or how the file builds the type variable.

# --- path: types_.py ---

from typing import AnyStr, ParamSpec, TypeAlias, TypeVar, TypeVarTuple, Union
import typing as t
import typing_extensions as te

class Foo:
    def method(self):
        pass

class Other:
    def other_method(self):
        pass

T = TypeVar("T")
Alias = Union[int, str]
Barred = int | str
Declared: TypeAlias = "int"
type Statement = int
P = ParamSpec("P")
Ts = TypeVarTuple("Ts")
K, V = TypeVar("K"), TypeVar("V")
Q = t.TypeVar("Q")
E = te.TypeVar("E")

def make_type_var(name):
    return TypeVar(name)

H = make_type_var("H")

# --- path: main.py ---

from types_ import T, Alias, Barred, Declared, Statement, P, Ts, K, V, Q, E, H, Foo, Other
from types_ import T as Renamed
from typing import AnyStr, Optional, TypeVar as TV, Union
import types_

def same(x: T) -> T:
    return Foo()
same(1).method
#       ^ defined: 12

def through_module(x: types_.T) -> types_.T:
    return Foo()
through_module(1).method
#                 ^ defined: 12

def renamed_import(x: Renamed) -> Renamed:
    return Foo()
renamed_import(1).method
#                 ^ defined: 12

def alias() -> Alias:
    return Foo()
alias().method
#       ^ defined: 12

def barred() -> Barred:
    return Foo()
barred().method
#        ^ defined: 12

def declared() -> Declared:
    return Foo()
declared().method
#          ^ defined: 12

def statement() -> Statement:
    return Foo()
statement().method
#           ^ defined: 12

def param_spec() -> P:
    return Foo()
param_spec().method
#            ^ defined: 12

def type_var_tuple() -> Ts:
    return Foo()
type_var_tuple().method
#                ^ defined: 12

def tuple_first(x: K) -> K:
    return Foo()
tuple_first(1).method
#              ^ defined: 12

def tuple_second(x: V) -> V:
    return Foo()
tuple_second(1).method
#               ^ defined: 12

def typing_attribute(x: Q) -> Q:
    return Foo()
typing_attribute(1).method
#                   ^ defined: 12

def extension(x: E) -> E:
    return Foo()
extension(1).method
#            ^ defined: 12

def helper(x: H) -> H:
    return Foo()
helper(1).method
#         ^ defined: 12

def any_string(x: AnyStr) -> AnyStr:
    return Foo()
any_string(1).method
#             ^ defined: 12

def string(x: T) -> "T":
    return Foo()
string(1).method
#         ^ defined: 12

def with_none(x: T) -> T | None:
    return Foo()
with_none(1).method
#            ^ defined: 12

def optional(x: T) -> Optional[T]:
    return Foo()
optional(1).method
#           ^ defined: 12

def union(x: T) -> Union[T, None]:
    return Foo()
union(1).method
#        ^ defined: 12

# A type variable in a container does not hide the container.
def container(x: T) -> list[T]:
    return Other()
container(1).other_method
#            ^ defined:

# A class that the file imports stays a class: the annotation hides the body.
def named() -> Other:
    return Foo()
named().method
#       ^ defined:
named().other_method
#       ^ defined: 16

# `typing_extensions` has no stub, so a TypeVar that it gives under another name is not seen:
# `from typing_extensions import TypeVar as ETV` and `T = ETV("T")` keep the annotation only.
# A type variable that the file builds with a renamed import.
Local = TV("Local")
def local(x: Local) -> Local:
    return Foo()
local(1).method
#        ^ defined: 12
