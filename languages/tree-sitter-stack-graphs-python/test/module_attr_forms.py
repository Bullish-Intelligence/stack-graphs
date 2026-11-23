# Names that a package or a module exports reach the code that reads them as attributes of the
# module. A name that a package imports (`from .m import Y`, `Y as V`, `Y as Y`) is such a name.

# --- path: pkg/__init__.py ---

# --- path: pkg/models/__init__.py ---

from ._known import KnownModelName as KnownModelName
from ._base import Model, Base as Renamed

# --- path: pkg/models/_known.py ---

KnownModelName = str

# --- path: pkg/models/_base.py ---

class Model:
    pass

class Base:
    pass

# --- path: pkg/build.py ---

def build():
    pass

class Result:
    pass

# --- path: pkg/agent/__init__.py ---

# --- path: pkg/agent/abstract.py ---

from .. import models, build
import pkg.models as pm
import pkg.build as pb
from pkg import models as other_models

def run(m: models.Model | models.KnownModelName | str | None = None) -> None:
    #             ^ defined: 9, 17
    #                                ^ defined: 8, 13
    models.Renamed
    #      ^ defined: 9, 20
    pm.Model
    #  ^ defined: 9, 17
    pm.KnownModelName
    #  ^ defined: 8, 13
    other_models.Renamed
    #            ^ defined: 9, 20
    build.build()
    #     ^ defined: 25
    build.Result
    #     ^ defined: 28
    pb.build()
    #  ^ defined: 25

# Negative: a name that the package does not export has no definition.

def missing() -> None:
    models.Absent
    #      ^ defined:
    build.Base
    #     ^ defined:
