#------ path: a/__init__.py ------#

foo = 1

#------ path: a/b/__init__.py ------#


bar = 2

#------ path: a/b/c.py ------#



X = 3

#------ path: main.py ------#

import a.b.c

print(a.foo, a.b.bar, a.b.c.X)
#       ^ defined: 3
#                ^ defined: 8
#                           ^ defined: 14
