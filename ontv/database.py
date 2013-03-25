import contextlib
import json
import os


class Block(object):
    def load_value(self, v):
        raise NotImplementedError("load_value")

    def dump_value(self, v):
        raise NotImplementedError("dump_value")

    def load_key(self, key):
        return key

    def dump_key(self, key):
        return key

    def write_header(self, fd):
        raise NotImplementedError("write_header")

    def read_header(self, fd):
        raise NotImplementedError("read_header")

    def load_item(self, i):
        raise NotImplementedError("load_item")

    def write_entry(self, fd, op, key, value):
        raise NotImplementedError("write_entry")

    def parse_entry(self, line):
        raise NotImplementedError("parse_entry")


class JSONValueBlock(object):
    def load_value(self, v):
        if v == "":
            return None

        try:
            return json.loads(v)
        except:
            return None

    def dump_value(self, v):
        if v is None:
            return ""

        try:
            return json.dumps(v)
        except:
            return ""


class StandardHeader(object):
    def write_header(self, fd):
        fd.write("version={0}".format(self.VERSION) + self.DELIM)

    def read_header(self, fd):
        v = fd.readline()
        v = v.strip()
        return dict(s.split("=", 1) for s in v.split(" "))


class V1Block(JSONValueBlock, StandardHeader, Block):
    SPACE = " "
    DELIM = "\n"
    VERSION = '1.0'

    def __init__(self, encoding='utf-8'):
        self._encoding = encoding

    def load_item(self, i):
        key, value = i
        return (self.load_key(key), self.load_value(value))

    def write_entry(self, fd, op, key, value):
        key = self.dump_key(key).encode(self._encoding)
        value = self.dump_value(value).encode(self._encoding)
        key_length = str(len(key))
        entry = self.SPACE.join((op, key_length, key, value)) + self.DELIM
        fd.write(entry)

    def parse_entry(self, line):
        op, key_length, b = line.split(self.SPACE, 2)
        key_length = int(key_length)
        key = b[:key_length].decode(self._encoding)
        value = b[key_length + 1:-1].decode(self._encoding)
        return op, key, value


class BlockYielder(object):
    def __init__(self, block_format, fd):
        self._block = block_format
        self._fd = fd

    def readblocks(self):
        if self._fd is None:
            raise Exception("file not open")

        try:
            for line in self._fd:
                op, key, value = self._block.parse_entry(line)
                key = self._block.load_key(key)
                yield op, key, value
        finally:
            self._fd.close()
            self._fd = None

    def load_item(self, i):
        return self._block.load_item(i)


class FilesystemDriver(object):
    block_formats = {
        V1Block.VERSION: V1Block,
    }

    def __init__(self, path, driver_open=open, encoding='utf-8'):
        self._path = path
        self._compact_path = os.path.join(
            os.path.dirname(path), u".{0}.compact".format(
                os.path.basename(path)))
        self._driver_open = driver_open
        self._encoding = encoding
        self._block = V1Block(encoding=self._encoding)

    def compact(self, entries):
        with self._driver_open(self._compact_path, 'w') as fd:
            self._block.write_header(fd)

            for op, key, value in entries:
                self._block.write_entry(fd, op, key, value)

        os.rename(self._compact_path, self._path)

    def appendlog(self, log):
        if not log:
            return

        with open(self._path, 'a') as fd:
            if fd.tell() == 0:
                self._write_header(fd)

            for op, key, value in log:
                fd.write(self._format_entry(op, key, value))

    def createyielder(self):
        fd = self._driver_open(self._path, 'r')

        header = self._block.read_header(fd)

        # setup temporary block format for reading.
        # this is useful for migrating old database formats behind the scenes.
        # The next time the database is compacted, the block format will
        # change.
        block_format = self.block_formats.get(header['version'])

        if block_format is None:
            raise Exception(
                "unknown block format version: {0}".format(header['version']))

        block_format = block_format(encoding=self._encoding)

        return BlockYielder(block_format, fd)


class DictStorage(object):
    # add a single value.
    VALUE = "+"

    # delete a single value.
    DELETION = "-"

    # clear the entire data set.
    CLEAR = "X"

    def __init__(self, path, driver):
        self._path = path
        self._log = list()
        self._driver = driver
        self._statistics = dict()

    def open(self):
        cache = self._read_cache(self._driver.createyielder())
        return cache, self._statistics

    def statistics(self):
        return self._statistics

    def _read_cache(self, yielder):
        data = dict()

        # generate statistics to decide if we should autocompact the database.
        clears = 0
        removes = 0

        for op, key, value in yielder.readblocks():
            if op == self.DELETION:
                data.pop(key, None)
                removes += 1
                continue

            if op == self.CLEAR:
                data.clear()
                clears += 1
                continue

            if op == self.VALUE:
                data[key] = value
                continue

            raise Exception("unknown operation '{0}'".format(op))

        self._statistics = {
            "clears": clears,
            "removes": removes,
            "nops": removes + clears,
        }

        return dict(yielder.load_item(i) for i in data.items())

    def commit(self):
        self._driver.appendlog(self._log)
        self._log = list()

    def setitem(self, key, value):
        self._log.append((self.VALUE, key, value))

    def delitem(self, key):
        self._log.append((self.DELETION, key, None))

    def clear(self, key):
        self._log.append((self.CLEAR, "", None))

    def compact(self, items):
        generator = ((self.VALUE, key, value) for key, value in items)

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

    def __setitem__(self, key, data):
        dict.__setitem__(self, key, data)
        self._storage.setitem(key, data)

    def __delitem__(self, key):
        dict.__delitem__(self, key)
        self._storage.delitem(key)

    def pop(self, key, *args, **kw):
        value = dict.pop(self, key, *args, **kw)
        self._storage.delitem(key)
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

    def list_append(self, key, data):
        array = list(self.get(key, []))
        array.append(data)
        self.__setitem__(key, array)

    def list_remove(self, key, data):
        array = self.get(key, [])
        array.remove(data)
        self.__setitem__(key, array)

    def compact(self):
        return self._storage.compact(self.items())

    def db_size(self):
        return self._storage.db_size()


class SetDB(set):
    def __init__(self, storage, cache):
        self._storage = storage
        set.__init__(self, cache.keys())

    def add(self, key):
        if key in self:
            return

        set.add(self, key)
        self._storage.setitem(key, None)

    def remove(self, key):
        set.remove(self, key)
        self._storage.delitem(key)

    def pop(self):
        key = set.pop(self)
        self._storage.delitem(key)
        return key

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
):
    driver_instance = driver(path, driver_open=driver_open)

    storage = DictStorage(path, driver_instance)

    cache, statistics = storage.open()

    db = impl(storage, cache)

    # nops means non-operations, basically operations that does not contribute
    # to the final structure of the data.
    #
    # if we hit a limit here, we should cleanup, otherwise it's a waste of
    # space.
    if statistics['nops'] > compaction_limit:
        db.compact()

    yield db
    storage.commit()
