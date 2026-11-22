# Forms of a type variable, an alias, and Any that the return annotation can name.
import typing as t
from typing import TypeVar as TV, AnyStr, Any, NoReturn
from typing import TypeVar as _TypeVar

class Foo:
    def method(self):
        pass

class Other:
    def other_method(self):
        pass

# G1: TypeVar through an alias of the module: the rule reads the word TypeVar after dots.
T1 = t.TypeVar("T1")
def g1(x: T1) -> T1:
    return Foo()
g1(1).method
#     ^ defined: 7

# G2: TypeVar imported under another name.
T2 = TV("T2")
def g2(x: T2) -> T2:
    return Foo()
g2(1).method
#     ^ defined: 7

# G3: TypeVar from typing without a bind in this file (AnyStr).
def g3(x: AnyStr) -> AnyStr:
    return Foo()
g3(1).method
#     ^ defined: 7

# G4: a TypeVar with a private import name.
T4 = _TypeVar("T4")
def g4(x: T4) -> T4:
    return Foo()
g4(1).method
#     ^ defined: 7

# G5: Any under another name.
from typing import Any as AnyType
def g5() -> AnyType:
    return Foo()
g5().method
#    ^ defined: 7

# G6: Typevar created in a helper call of this file.
def make_tv(name):
    return TV(name)
T6 = make_tv("T6")
def g6(x: T6) -> T6:
    return Foo()
g6(1).method
#     ^ defined: 7

# G7: Union with an alias of this file written with a bar and a call on the right.
Num = int | float
def g7() -> Num:
    return Foo()
g7().method
#    ^ defined: 7

# G8: NoReturn and Never (no class).
def g8() -> NoReturn:
    raise ValueError

# G9: Annotated with TypeVar metadata.
T9 = t.TypeVar("T9")
def g9(x: T9) -> t.Annotated[T9, "meta"]:
    return Foo()
g9(1).method
#     ^ defined: 7

# G10: nested Optional/List/Optional of a TypeVar.
T10 = t.TypeVar("T10")
def g10(x: T10) -> t.Optional[t.Optional[T10]]:
    return Foo()
g10(1).method
#      ^ defined: 7

# G11: Self in a string with a bar.
class B11:
    def configure(self): pass
    def chain(self) -> "B11 | None":
        return self
B11().chain().configure
#              ^ defined: 84

# G12: a string with a dotted name.
def g12() -> "t.Any":
    return Foo()
g12().method
#     ^ defined: 7

# G13: a TypeVar of a class body used in a method.
class G13:
    T = t.TypeVar("T")
    def m(self, x: "T") -> "T":
        return Foo()
G13().m(1).method
#          ^ defined: 7
