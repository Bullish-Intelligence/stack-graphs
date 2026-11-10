#------ path: pkg/sub/deep/main.py ------#

from .near import N
#                 ^ defined: 27
from ..mid import M
#                 ^ defined: 32
from ...top import T
#                  ^ defined: 37
from ... import top
#               ^ defined: 36
from ...top.inner import I
#                        ^ defined: 42
# An import above the root of the project does not resolve.
from .... import root_level
#                ^ defined:
from . import near

print(N, M, T, I)
#     ^ defined: 3, 27
#        ^ defined: 5, 32
#           ^ defined: 7, 37
#              ^ defined: 11, 42

#------ path: pkg/sub/deep/near.py ------#

# module
N = 1

#------ path: pkg/sub/mid.py ------#

# module
M = 2

#------ path: pkg/top.py ------#

# module
T = 3

#------ path: pkg/top/inner.py ------#

# module
I = 4

#------ path: root_level.py ------#

# module
