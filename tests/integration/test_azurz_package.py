"""Le wheel publié embarque le pont Azurz et son interface."""

import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_wheel_ships_azurz_and_the_web_ui(tmp_path: Path) -> None:
    subprocess.run(
        ["uv", "build", "--wheel", "--out-dir", str(tmp_path)],
        cwd=ROOT,
        check=True,
    )
    wheel = next(tmp_path.glob("*.whl"))
    with zipfile.ZipFile(wheel) as archive:
        names = set(archive.namelist())
        entry_path = next(name for name in names if name.endswith("/entry_points.txt"))
        entry = archive.read(entry_path).decode()
    assert "azurz/__init__.py" in names
    assert "azurz/web/static/index.html" in names
    assert "azurz/web/static/styles.css" in names
    assert "azurz/web/static/app.js" in names
    assert "car_compare/__init__.py" in names
    assert "azurz = azurz.main:main" in entry
    assert "azurz-azdo = azurz.labazdo.app:main" in entry
