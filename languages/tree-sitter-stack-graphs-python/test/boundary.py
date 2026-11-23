# The flow of a function finds a parameter before a hoisted local of the same name. The hoisted
# locals come next, and a local name never reaches the enclosing scope. A signature annotation and
# a default value stay outside the function, so they never see a local that a later import binds.

import signal

class Handler(object):
    def __init__(self, default_handler):
        self.called = False
        self.original_handler = default_handler
#                               ^ defined: 8
        if isinstance(default_handler, int):
#                     ^ defined: 8
            if default_handler == signal.SIG_DFL:
#              ^ defined: 8
                default_handler = signal.default_int_handler
            elif default_handler == signal.SIG_IGN:
#                ^ defined: 8
                def default_handler(unused_signum, unused_frame):
                    raise TypeError("unexpected")
            else:
                raise TypeError("expected")
        self.default_handler = default_handler
#                              ^ defined: 8, 16, 19

from os import path as os_path

def outer(first: os_path.Foo = os_path):
#                ^ defined: 26
#                              ^ defined: 26
    from posixpath import path as os_path
    return os_path
#          ^ defined: 31

value = 1

def local_value():
    print(value)
#         ^ defined: 40
    value = 2
