class A:
    w = 1

    @property
    def p(self):
        return self.w
#                   ^ defined: 2

    @classmethod
    def c(cls):
        cls.z = 2
        return cls.w, cls().p, cls.p, cls.z
#                  ^ defined: 2
#                           ^ defined: 5
#                                  ^ defined: 5
#                                         ^ defined: 11

    @staticmethod
    def s(x):
        return x.w

    @custom.decorator
    def d(self):
        self.q = 3
        return self.q, self.p
#                   ^ defined: 24
#                           ^ defined: 5
