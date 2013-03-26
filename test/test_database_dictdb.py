import unittest
import mock

from ontv.database import DictDB


class TestDatabaseDictDB(unittest.TestCase):
    def test_setitem(self):
        key = object()
        value = object()
        storage = mock.Mock()
        d = DictDB(storage, {})
        d[key] = value
        storage.setitem.assert_called_with(key, value)

    def test_delitem(self):
        key = object()
        value = object()
        storage = mock.Mock()
        d = DictDB(storage, {key: value})
        del d[key]
        storage.delitem.assert_called_with(key)

    def test_pop(self):
        key = object()
        value = object()
        storage = mock.Mock()
        d = DictDB(storage, {key: value})
        self.assertEquals(value, d.pop(key))
        storage.delitem.assert_called_with(key)

    def test_clear(self):
        key = object()
        value = object()
        storage = mock.Mock()
        d = DictDB(storage, {key: value})
        d.clear()
        self.assertTrue(storage.clear.called)

    def test_popitem(self):
        key = object()
        value = object()
        storage = mock.Mock()
        d = DictDB(storage, {key: value})
        self.assertEquals((key, value), d.popitem())
        storage.delitem.assert_called_with(key)

    def test_list_append(self):
        key = object()
        value = object()
        storage = mock.Mock()
        d = DictDB(storage, {key: []})
        d.list_append(key, value)
        storage.setitem.assert_called_with(key, [value])

    def test_list_remove(self):
        key = object()
        value = object()
        storage = mock.Mock()
        d = DictDB(storage, {key: [value]})
        d.list_remove(key, value)
        storage.setitem.assert_called_with(key, [])

    def test_compact(self):
        key = object()
        value = object()
        storage = mock.Mock()
        stats = storage.compact.return_value
        d = DictDB(storage, {key: value})
        self.assertEquals(stats, d.compact())
        storage.compact.assert_called_with([(key, value)])

    def test_db_size(self):
        storage = mock.Mock()
        size = storage.db_size.return_value
        d = DictDB(storage, {})
        self.assertEquals(size, d.db_size())
