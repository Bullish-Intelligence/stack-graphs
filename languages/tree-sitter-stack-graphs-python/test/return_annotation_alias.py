# A function that a return annotation names as a type alias keeps the value following of its
# `return` statements. The alias can be spelled `TypeAlias`, or `_TypeAlias` as in mypy.

from typing import Optional
from typing_extensions import TypeAlias as _TypeAlias

class A:
    def member(self):
        pass

Alias: _TypeAlias = Optional[A]

def make() -> Alias:
    return A()

make().member
#      ^ defined: 8
