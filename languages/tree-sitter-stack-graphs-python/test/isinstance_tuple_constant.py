# `isinstance(x, NAMES)` narrows `x` to each class of the tuple that NAMES holds.
# The tuple is in the same file or in an imported module.

# --- path: nodes.py ---

class FuncDef:
    def is_class(self):
        pass

class Decorator:
    def is_class(self):
        pass
    def only_decorator(self):
        pass

class Other:
    def other_only(self):
        pass

FUNC_TYPES = (FuncDef, Decorator)
TUPLE_WITHOUT_PARENS = FuncDef, Decorator

# --- path: local.py ---

class A:
    def a_only(self):
        pass

class B:
    def b_only(self):
        pass

LOCAL_TYPES = (A, B)

def same_file(x):
    if isinstance(x, LOCAL_TYPES):
        x.a_only
        # ^ defined: 26
        x.b_only
        # ^ defined: 30
    x.a_only
    # ^ defined:

def not_a_tuple(x):
    if isinstance(x, A):
        x.a_only
        # ^ defined: 26
        x.b_only
        # ^ defined:

# --- path: main.py ---

import nodes
from nodes import FUNC_TYPES, TUPLE_WITHOUT_PARENS, Other

def imported(x):
    if isinstance(x, FUNC_TYPES):
        x.is_class
        # ^ defined: 7, 11
        x.only_decorator
        # ^ defined: 13
        x.other_only
        # ^ defined:
    x.only_decorator
    # ^ defined:

def without_parens(x):
    if isinstance(x, TUPLE_WITHOUT_PARENS):
        x.only_decorator
        # ^ defined: 13

def dotted(x):
    if isinstance(x, nodes.FUNC_TYPES):
        x.only_decorator
        # ^ defined: 13

def assert_guard(x):
    assert isinstance(x, FUNC_TYPES)
    x.only_decorator
    # ^ defined: 13

def and_chain(x, y):
    if y and isinstance(x, FUNC_TYPES) and x.only_decorator:
        #                                    ^ defined: 13
        pass

def other_class(x):
    if isinstance(x, Other):
        x.other_only
        # ^ defined: 17
