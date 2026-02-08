"""Shared fixtures for LaminarDB tests."""

import pytest

import laminardb

# ---------------------------------------------------------------------------
# Skip markers for optional dependencies
# ---------------------------------------------------------------------------

try:
    import pandas as pd

    HAS_PANDAS = True
except ImportError:
    HAS_PANDAS = False

try:
    import polars as pl

    HAS_POLARS = True
except ImportError:
    HAS_POLARS = False

try:
    import pyarrow as pa

    HAS_PYARROW = True
except ImportError:
    HAS_PYARROW = False

requires_pandas = pytest.mark.skipif(not HAS_PANDAS, reason="pandas not installed")
requires_polars = pytest.mark.skipif(not HAS_POLARS, reason="polars not installed")
requires_pyarrow = pytest.mark.skipif(not HAS_PYARROW, reason="pyarrow not installed")


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def db(tmp_path):
    """Open a LaminarDB connection with a test source."""
    conn = laminardb.open(str(tmp_path / "test.db"))
    conn.create_table(
        "sensors",
        {"ts": "int64", "device": "string", "value": "float64"},
    )
    yield conn
    conn.close()


@pytest.fixture
def sample_data():
    """Sample sensor data in various formats."""
    return {
        "row": {"ts": 1, "device": "sensor_a", "value": 42.0},
        "rows": [
            {"ts": 1, "device": "sensor_a", "value": 42.0},
            {"ts": 2, "device": "sensor_b", "value": 43.5},
            {"ts": 3, "device": "sensor_a", "value": 44.1},
        ],
        "columnar": {
            "ts": [1, 2, 3],
            "device": ["sensor_a", "sensor_b", "sensor_a"],
            "value": [42.0, 43.5, 44.1],
        },
        "json": '[{"ts": 1, "device": "sensor_a", "value": 42.0}]',
        "csv": "ts,device,value\n1,sensor_a,42.0\n2,sensor_b,43.5\n",
    }


@pytest.fixture
async def async_db(tmp_path):
    """Open a LaminarDB connection for async tests."""
    conn = laminardb.open(str(tmp_path / "async_test.db"))
    conn.create_table(
        "sensors",
        {"ts": "int64", "device": "string", "value": "float64"},
    )
    yield conn
    conn.close()
