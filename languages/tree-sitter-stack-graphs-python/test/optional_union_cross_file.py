# A union alias that another file defines, and a quoted union that names an imported class.

# --- path: models.py ---
from typing import Optional, Union

class Request:
    def instructions(self):
        pass

class Response:
    def model_name(self):
        pass

class Part:
    def content(self):
        pass

Message = Request | Response
Choice = Union[Request, Part]
Maybe = Optional[Part]

# --- path: main.py ---
from models import Message, Choice, Maybe, Request, Response
import models

def use(a: Message, b: Choice, c: Maybe, d: "Request | None", e: models.Message):
    a.instructions
    #  ^ defined: 7
    a.model_name
    #  ^ defined: 11
    b.content
    #  ^ defined: 15
    b.instructions
    #  ^ defined: 7
    c.content
    #  ^ defined: 15
    d.instructions
    #  ^ defined: 7
    e.model_name
    #  ^ defined: 11
    a.content
    #  ^ defined:
