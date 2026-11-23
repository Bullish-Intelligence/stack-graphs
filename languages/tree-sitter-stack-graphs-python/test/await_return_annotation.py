# `await f()` gives the value that the call returns. For an `async def` with a return annotation,
# the annotation names the awaited type.

class Result:
    code: int
    def run(self):
        pass

class Other:
    code: int

async def make() -> Result:
    return Other()

async def make_plain():
    return Result()

# an awaited call of a function
async def use_function():
    x = await make()
    x.code
    # ^ defined: 5
    x.run
    # ^ defined: 6

    (await make()).code
    #              ^ defined: 5

    y = await make_plain()
    y.code
    # ^ defined: 5

# an awaited call of a method of self
class Client:
    async def fetch(self) -> Result:
        return Result()

    async def go(self):
        r = await self.fetch()
        r.code
        # ^ defined: 5
        return await self.fetch()

# an awaited call of a method of an annotated receiver
async def use_receiver(c: Client, d: "Client"):
    r = await c.fetch()
    r.code
    # ^ defined: 5
    s = await d.fetch()
    s.code
    # ^ defined: 5

# an awaited method of an object that a call makes
async def use_instance():
    c = Client()
    r = await c.fetch()
    r.code
    # ^ defined: 5
    c.go
    # ^ defined: 38

# the awaited value passes through a return
async def forward():
    return await make()

async def use_forward():
    z = await forward()
    z.code
    # ^ defined: 5

# a comprehension
async def use_comprehension(c: Client):
    [(await c.fetch()).code for _ in range(3)]
    #                   ^ defined: 5

# an awaited method of an instance that a call makes in the expression
async def use_class_value():
    w = await Client().fetch()
    w.code
    # ^ defined: 5

# a name that the awaited call does not give
async def negative(c: Client):
    r = await c.fetch()
    r.go
    # ^ defined:
    r.nothing
    # ^ defined:
