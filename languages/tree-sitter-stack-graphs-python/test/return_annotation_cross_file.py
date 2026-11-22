# A TypeVar or an alias that another file defines, and a TypeVar that a file builds in other ways.

# --- path: types_.py ---
from typing import TypeVar, Union
T = TypeVar("T")
Alias = Union[int, str]
class Foo:
    def method(self):
        pass

# --- path: main.py ---
from types_ import T, Foo, Alias
import types_

def f(x: T) -> T:
    return Foo()
f(1).method
#    ^ defined: 8

def g(x: types_.T) -> types_.T:
    return Foo()
g(1).method
#    ^ defined: 8

def h() -> Alias:
    return Foo()
h().method
#   ^ defined: 8

def i(x: T) -> "T":
    return Foo()
i(1).method
#    ^ defined: 8

def j(x: T) -> T | None:
    return Foo()
j(1).method
#    ^ defined: 8

def k(x: T) -> list[T]:
    return Foo()
k(1).method
#    ^ defined:
