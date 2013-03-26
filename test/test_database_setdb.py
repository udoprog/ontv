import unittest
import mock

from ontv.database import SetDB


class TestDatabaseSetDB(unittest.TestCase):
    def test_add(self):
        key = object()
        storage = mock.Mock()
        s = SetDB(storage, {})
        s.add(key)
        storage.setitem.assert_called_with(key, None)

    def test_remove(self):
        key = object()
        storage = mock.Mock()
        s = SetDB(storage, {key: None})
        s.remove(key)
        storage.delitem.assert_called_with(key)

    def test_pop(self):
        key = object()
        storage = mock.Mock()
        s = SetDB(storage, {key: None})
        self.assertEquals(key, s.pop())
        storage.delitem.assert_called_with(key)

    def test_compact(self):
        key = object()
        storage = mock.Mock()
        stats = storage.compact.return_value
        s = SetDB(storage, {key: None})
        self.assertEquals(stats, s.compact())
        self.assertTrue(storage.compact.called)
        self.assertItemsEqual([(key, None)], storage.compact.call_args[0][0])

    def test_db_size(self):
        storage = mock.Mock()
        size = storage.db_size.return_value
        s = SetDB(storage, {})
        self.assertEquals(size, s.db_size())
