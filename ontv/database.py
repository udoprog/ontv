import json
import contextlib
import os


class JsonBlockFormat(object):
    def load(self, v):
        if v == "":
            return None

        try:
            return json.loads(v)
        except:
            return None

    def dump(self, v):
        if v is None:
            return ""

        try:
            return json.dumps(v)
        except:
            return ""


class FilesystemDriver(object):
    SPACE = " "
    DELIM = "\n"

    def __init__(self, path, driver_open=open):
        self._path = path
        self._compact_path = os.path.join(
            os.path.dirname(path), u".{0}.compact".format(
                os.path.basename(path)))
        self._driver_open = driver_open

    def _format_entry(self, op, ident, block):
        return self.SPACE.join((op, ident, block)) + self.DELIM

    def open(self):
        self._fd = self._driver_open(self._path, 'a+')

    def close(self):
        if not self._fd:
            raise Exception("file not open")

        self._fd.close()
        self._fd = None

    def reopen(self):
        if self._fd:
            self._fd.close()

        self._fd = self._driver_open(self._path, 'a+')

    def compact(self, entries):
        if not self._fd:
            raise Exception("file not open")

        with self._driver_open(self._compact_path, 'w') as fd:
            for op, ident, block in entries:
                fd.write(self._format_entry(op, ident, block))

        os.rename(self._compact_path, self._path)
        self.reopen()

    def append(self, op, ident, block):
        self._fd.write(self._format_entry(op, ident, block))

    def yieldentries(self):
        if not self._fd:
            raise Exception("file not open")

        self._fd.seek(0)

        for line in self._fd:
            op, ident, block = line.split(self.SPACE, 2)
            yield op, ident, block

    def valid_id(self, ident):
        if self.SPACE in ident:
            raise Exception("identifier may not contain a space")

        if self.DELIM in ident:
            raise Exception("identifier may not contain a newline")

    def db_size(self):
        return self._fd.size()


class DictStorage(object):
    PUT = "+"
    REMOVE = "-"
    CLEAR = "X"

    def __init__(self, path, driver, block):
        self._path = path
        self._fd = None
        self._log = list()
        self._driver = driver
        self._block = block
        self._statistics = dict()

    def open(self):
        self._driver.open()
        self._fd = open(self._path, 'a+')
        cache = self._read_cache(self._driver.yieldentries())
        return cache, self._statistics

    def statistics(self):
        return self._statistics

    def _read_cache(self, entries):
        if not self._fd:
            raise Exception("file not open")

        data = dict()

        # generate statistics to decide if we should autocompact the database.
        clears = 0
        removes = 0

        for op, ident, block in entries:
            if op == self.REMOVE:
                data.pop(ident, None)
                removes += 1
                continue

            if op == self.PUT:
                data[ident] = block
                continue

            if op == self.CLEAR:
                data.clear()
                clears += 1
                continue

            raise Exception("unknown operation '{0}'".format(op))

        self._statistics = {
            "clears": clears,
            "removes": removes,
            "nops": removes + clears,
        }

        return (dict((k, self._block.load(v)) for k, v in data.items()))

    def _append_log(self, log):
        for op, ident, block in log:
            block = self._block.dump(block)
            self._driver.append(op, ident, block)

    def commit(self):
        self._append_log(self._log)
        self._log = list()

    def close(self):
        self._driver.close()

    def valid_id(self, ident):
        return self._driver.valid_id(ident)

    def setitem(self, ident, block):
        self._log.append((self.PUT, ident, block))

    def delitem(self, ident):
        self._log.append((self.REMOVE, ident, None))

    def clear(self, ident):
        self._log.append((self.CLEAR, "", None))

    def compact(self, items):
        generator = ((self.PUT, ident, self._block.dump(block))
                     for ident, block in items)
        self._driver.compact(generator)

        statistics = self._statistics

        self._statistics = {
            'clears': 0,
            'removes': 0,
            'nops': 0,
        }

        return statistics

    def db_size(self):
        return self._driver.db_size()


class DictDB(dict):
    def __init__(self, storage, cache):
        self._storage = storage
        dict.__init__(self, cache)

    def __setitem__(self, ident, data):
        self._storage.valid_id(ident)
        dict.__setitem__(self, ident, data)
        self._storage.setitem(ident, data)

    def __delitem__(self, ident):
        self._storage.valid_id(ident)
        dict.__delitem__(self, ident)
        self._storage.delitem(ident)

    def pop(self, ident, *args, **kw):
        self._storage.valid_id(ident)
        value = dict.pop(self, ident, *args, **kw)
        self._storage.delitem(ident)
        return value

    def clear(self):
        dict.clear(self)
        self._storage.clear()

    def setdefault(self, **args):
        raise NotImplementedError("setdefault")

    def update(self, **args):
        raise NotImplementedError("update")

    def popitem(self, **args):
        raise NotImplementedError("popitem")

    def list_append(self, ident, data):
        array = list(self.get(ident, []))
        array.append(data)
        self.__setitem__(ident, array)

    def list_remove(self, ident, data):
        array = self.get(ident, [])
        array.remove(data)
        self.__setitem__(ident, array)

    def compact(self):
        return self._storage.compact(self.items())

    def db_size(self):
        return self._storage.db_size()


class SetDB(set):
    def __init__(self, storage, cache):
        self._storage = storage
        set.__init__(self, cache.keys())

    def add(self, ident):
        if ident in self:
            return

        self._storage.valid_id(ident)
        set.add(self, ident)
        self._storage.setitem(ident, None)

    def remove(self, ident):
        self._storage.valid_id(ident)
        set.remove(self, ident)
        self._storage.delitem(ident)

    def pop(self):
        ident = set.pop(self)
        self._storage.delitem(ident)
        return ident

    def compact(self):
        return self._storage.compact((v, None) for v in self)

    def db_size(self):
        return self._storage.db_size()


@contextlib.contextmanager
def open_database(
    path,
    compaction_limit=1000,
    impl=DictDB,
    driver_open=open,
    driver=FilesystemDriver,
    block_format=JsonBlockFormat
):
    driver_instance = driver(path, driver_open=driver_open)
    block = block_format()

    storage = DictStorage(path, driver_instance, block)

    cache, statistics = storage.open()

    db = impl(storage, cache)

    # nops means non-operations, basically operations that does not contribute
    # to the final structure of the data.
    #
    # if we hit a limit here, we should cleanup, otherwise it's a waste of
    # space.
    if statistics['nops'] > compaction_limit:
        db.compact()

    try:
        yield db
        storage.commit()
    finally:
        storage.close()
