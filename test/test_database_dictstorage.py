import unittest
import mock

from ontv.database import DictStorage


class TestDictStorage(unittest.TestCase):
    @mock.patch.object(DictStorage, '_read_cache')
    def test_read(self, read_cache):
        stats = object()
        ref_data = object()

        read_cache.return_value = (stats, ref_data)

        driver = mock.Mock()
        storage = DictStorage(driver)

        # just initialized storage has no stats.
        self.assertEquals(None, storage.stats())
        self.assertEquals(ref_data, storage.read())
        self.assertEquals(stats, storage.stats())

    def test_commit(self):
        ref_log = []

        driver = mock.Mock()
        storage = DictStorage(driver)
        storage.commit()

        driver.appendlog.assert_called_with([])
        self.assertEquals(ref_log, storage._log)

    def test_setitem(self):
        key = object()
        value = object()

        ref_log = [
            (DictStorage.VALUE, key, value)
        ]

        driver = mock.Mock()
        storage = DictStorage(driver)
        storage.setitem(key, value)

        self.assertEquals(ref_log, storage._log)

    def test_delitem(self):
        key = object()

        ref_log = [
            (DictStorage.DELETION, key, None)
        ]

        driver = mock.Mock()
        storage = DictStorage(driver)
        storage.delitem(key)

        self.assertEquals(ref_log, storage._log)

    def test_clear(self):
        key = object()

        ref_log = [
            (DictStorage.CLEAR, "", None)
        ]

        driver = mock.Mock()
        storage = DictStorage(driver)
        storage.clear(key)

        self.assertEquals(ref_log, storage._log)
