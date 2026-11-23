# A type alias of a container gives the element type to a name that it annotates.
# The alias can be in the same file, or in another file.

# --- path: build.py ---

from typing import Dict, List, Optional, Union
from typing_extensions import TypeAlias as _TypeAlias

class State:
    tree = 1

class Other:
    tree = 2

Graph: _TypeAlias = dict[str, State]
Quoted: _TypeAlias = dict[str, "State"]
Items = List[State]
Maybe = Optional[State]
Either = Union[State, Other]
Piped = State | Other
REGISTRY = {"a": State()}
ITEM = REGISTRY["a"]

# --- path: update.py ---

from build import Graph, Quoted, Items, Maybe, Either, Piped, ITEM
import build

def f(graph: Graph, quoted: Quoted, items: Items, other: build.Graph):
    state = graph["a"]
    state.tree
    #     ^ defined: 10
    quoted["a"].tree
    #           ^ defined: 10
    for item in items:
        item.tree
        #     ^ defined: 10
    for value in graph.values():
        value.tree
        #     ^ defined: 10
    other["a"].tree
    #          ^ defined: 10

def g(maybe: Maybe, either: Either, piped: Piped):
    maybe.tree
    #     ^ defined: 10
    either.tree
    #      ^ defined: 10, 13
    piped.tree
    #     ^ defined: 10, 13

# Negatives: a name with no annotation gives nothing, and a variable that holds a subscript of
# another variable is no type alias.

def h(plain):
    plain["a"].tree
    #          ^ defined:

def i(item: ITEM):
    item.tree
    #    ^ defined:
