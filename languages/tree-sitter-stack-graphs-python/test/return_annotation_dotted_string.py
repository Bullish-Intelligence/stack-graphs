# A dotted name in a string, like `-> "mod.Foo"`, has no annotation rule. The gate keeps the body
# route, so the call follows the returned values.

# --- path: mod.py ---

class Foo:
    def method(self):
        pass

class Other:
    def other_method(self):
        pass

# --- path: main.py ---

from typing import Optional, Union
import mod

def dotted() -> "mod.Foo":
    return mod.Foo()

dotted().method
#        ^ defined: 7

def optional_dotted() -> Optional["mod.Foo"]:
    return mod.Foo()

optional_dotted().method
#                 ^ defined: 7

def union_dotted() -> Union["mod.Foo", None]:
    return mod.Foo()

union_dotted().method
#              ^ defined: 7

def single_quotes() -> 'mod.Foo':
    return mod.Foo()

single_quotes().method
#               ^ defined: 7

# The body decides. A wrong value gives a wrong member, as for any annotation with no class.
def other_value() -> "mod.Foo":
    return mod.Other()

other_value().other_method
#             ^ defined: 11

# A plain name in a string still follows the annotation only.
class Local:
    def local_method(self):
        pass

def plain_name() -> "Local":
    return mod.Foo()

plain_name().local_method
#            ^ defined: 52
plain_name().method
#            ^ defined:
