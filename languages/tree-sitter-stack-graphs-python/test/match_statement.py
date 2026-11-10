from dataclasses import dataclass
from enum import Enum

class Color(Enum):
    RED = 1

@dataclass
class Point:
    x: int
    y: int

def helper(value):
    return value

def handle(event, color):
    match helper(event):
#         ^ defined: 12
#                ^ defined: 15
        case Point(x=0, y=yy) if yy > 0:
#            ^ defined: 8
#                  ^ defined: 9
#                       ^ defined: 10
#                                ^ defined: 19
            return helper(yy)
#                  ^ defined: 12
#                         ^ defined: 19
        case Color.RED:
#            ^ defined: 4
#                  ^ defined: 5
            return color
#                  ^ defined: 15
        case [first, *rest]:
            return helper(first), rest
#                  ^ defined: 12
#                         ^ defined: 32
#                                 ^ defined: 32
        case {"k": v, **kw}:
            return v, kw
#                  ^ defined: 37
#                     ^ defined: 37
        case Point() as pt:
#            ^ defined: 8
            return pt
#                  ^ defined: 41
        case captured:
            return captured
#                  ^ defined: 45
        case _:
            return helper(event)
#                  ^ defined: 12
#                         ^ defined: 15

match helper(1):
#     ^ defined: 12
    case Point(x=1) as module_pt:
#        ^ defined: 8
#              ^ defined: 9
        pass
    case module_capture:
        pass
print(module_pt, module_capture)
#     ^ defined: 55
#                ^ defined: 59

def after(event):
    match event:
        case [a, b]:
            pass
    return a, b
#          ^ defined: 67
#             ^ defined: 67


x = 7

def keyword_is_no_binding(event):
    match event:
        case Point(x=0):
#            ^ defined: 8
#                  ^ defined: 9
            return x
#                  ^ defined: 74


def other_patterns(event):
    match event:
        case 1 | Color.RED:
#                ^ defined: 4
#                      ^ defined: 5
            pass
        case {Color.RED: found}:
#             ^ defined: 4
#                   ^ defined: 5
            return found
#                  ^ defined: 91
        case Point(x=Color.RED, y=1):
#            ^ defined: 8
#                  ^ defined: 9
#                    ^ defined: 4
#                          ^ defined: 5
#                               ^ defined: 10
            pass
