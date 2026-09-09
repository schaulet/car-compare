"""API v1 routers."""

from fastapi import APIRouter

from .comparisons import router as comparisons_router
from .simulations import router as simulations_router
from .vehicles import router as vehicles_router

router = APIRouter()
router.include_router(vehicles_router, prefix="/vehicles", tags=["vehicles"])
router.include_router(simulations_router, prefix="/simulations", tags=["simulations"])
router.include_router(comparisons_router, prefix="/comparisons", tags=["comparisons"])
