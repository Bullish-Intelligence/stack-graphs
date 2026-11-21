# A return annotation that names a class hides the returns of the body.
# A return annotation that names no class does not. The call follows the returned values then.
from typing import Any, Optional, Self, Type, TypeAlias, TypeVar, Union

class Foo:
    def method(self):
        pass

class Other:
    def other_method(self):
        pass

# A class annotation shadows the body.
def named() -> Foo:
    return Other()

named().method
#       ^ defined: 6
named().other_method
#       ^ defined:

# A TypeVar that the file binds.
T = TypeVar("T")

def typevar_return(x: T) -> T:
    return Foo()

typevar_return(1).method
#                 ^ defined: 6

def optional_typevar(x: T) -> Optional[T]:
    return Foo()

optional_typevar(1).method
#                   ^ defined: 6

def union_typevar(x: T) -> Union[T, None]:
    return Foo()

union_typevar(1).method
#                ^ defined: 6

def string_typevar(x: T) -> "T":
    return Foo()

string_typevar(1).method
#                 ^ defined: 6

# A TypeVar in a container does not hide the container.
def list_of_typevar(x: T) -> list[T]:
    return Other()

list_of_typevar(1).other_method
#                  ^ defined:

# A ParamSpec and a bound TypeVar.
F = TypeVar("F", bound="Foo")

def bound_return(x: F) -> F:
    return Foo()

bound_return(1).method
#               ^ defined: 6

# Self.
class Builder:
    def configure(self):
        pass

    def chain(self) -> Self:
        return self

Builder().chain().configure
#                 ^ defined: 67

# Any.
def any_return() -> Any:
    return Foo()

any_return().method
#            ^ defined: 6

def optional_any() -> Optional[Any]:
    return Foo()

optional_any().method
#              ^ defined: 6

# Any inside a container does not hide the container.
def dict_of_any() -> dict[str, Any]:
    return Other()

dict_of_any().other_method
#             ^ defined:

# Type[...] and type[...].
def type_return() -> Type[Foo]:
    return Foo

type_return().method
#             ^ defined: 6

def lowercase_type_return() -> type[Foo]:
    return Foo

lowercase_type_return().method
#                       ^ defined: 6

# A type alias.
Alias: TypeAlias = "Foo"
Subscripted = Union[Foo, None]
Barred = Foo | None
type Statement = Foo

def alias_return() -> Alias:
    return Foo()

alias_return().method
#              ^ defined: 6

def subscripted_return() -> Subscripted:
    return Foo()

subscripted_return().method
#                    ^ defined: 6

def barred_return() -> Barred:
    return Foo()

barred_return().method
#               ^ defined: 6

def statement_return() -> Statement:
    return Foo()

statement_return().method
#                  ^ defined: 6

# A type parameter.
def first[U](items: list[U]) -> U:
    return Foo()

first([]).method
#         ^ defined: 6

class Box[V]:
    def get(self) -> V:
        return Foo()

Box().get().method
#           ^ defined: 6

# A string annotation keeps the value following next to the annotation. A store like `o.x = x`
# after `o = self.copy()` needs the route of the body to reach the member `x`.
class Options:
    min_width: int
    height: int

    def copy(self) -> "Options":
        options: Options = Options.__new__(Options)
        return options

    def update(self, min_width, height):
        options = self.copy()
        options.min_width = min_width
        #       ^ defined: 156, 163, 165
        options.min_width = min_width
        #       ^ defined: 156, 163, 165, 167
        options.height = options.min_width = height
        #       ^ defined: 157, 163, 169
        return options

def optional_string(options: Options) -> Optional["Options"]:
    return Options()

optional_string(Options()).min_width
#                          ^ defined: 156

def union_string(options: Options) -> "Options | None":
    return Options()

union_string(Options()).min_width
#                       ^ defined: 156

def dotted_string() -> "Options":
    return Options()

dotted_string().min_width
#               ^ defined: 156

def dotted_union_string() -> "typing.Optional[Options]":
    return Options()

dotted_union_string().min_width
#                     ^ defined: 156

def plain_string_names_a_class() -> "Options":
    return Other()

plain_string_names_a_class().min_width
#                            ^ defined: 156
plain_string_names_a_class().other_method
#                            ^ defined: 10
