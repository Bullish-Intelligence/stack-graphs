# A relative import in the __init__.py of a package binds the submodule as a name of the package.

# --- path: pkg/__init__.py ---

from .base import *
from .sub.inner import thing
from .same import same

names = base.NAMES
#            ^ defined: 18
more = sub.VALUE
#          ^ defined: 29
other = same.attribute
#            ^ defined: 25

# --- path: pkg/base.py ---

NAMES = 1

# --- path: pkg/same.py ---

def same():
    pass

same.attribute = 1

# --- path: pkg/sub/__init__.py ---

VALUE = 2

# --- path: pkg/sub/inner.py ---

thing = 3

# --- path: pkg/plain.py ---

from .base import *

# A module that is no __init__.py does not bind the submodule.

late = base.NAMES
#           ^ defined:

# --- path: outer.py ---

import pkg

# A file that imports the package reads the submodule as an attribute.

pkg.base.NAMES
#        ^ defined: 18
pkg.sub.VALUE
#       ^ defined: 29
