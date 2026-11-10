# A keyword argument never names `*args` or `**kwargs`.
def f(a, *args, b=1, **kwargs):
    pass

f(1, b=2)
#    ^ defined: 2
f(1, c=2)
#    ^ defined:
f(1, args=2)
#    ^ defined:
f(1, kwargs=2)
#    ^ defined:

class C:
    def __init__(self, *args, **kwargs):
        pass

C(args=1)
# ^ defined:
