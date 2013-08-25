import contextlib
import json
import os
import collections


DictStorageStats = collections.namedtuple(
    "DictStorageStats",
    ['clears', 'deletions', 'noops'])


def json_load(v):
    if v == "":
        return None

    try:
        return json.loads(v)
    except:
        return None


def json_dump(v):
    if v is None:
        return ""

    try:
        return json.dumps(v)
    except:
        return ""


class Block(object):
    def load_value(self, v):
        raise NotImplementedError("load_value")

    def dump_value(self, v):
        raise NotImplementedError("dump_value")

    def load_key(self, key):
        return key

    def dump_key(self, key):
        return key

    def dump_header(self, fd):
        raise NotImplementedError("dump_header")

    def load_header(self, fd):
        raise NotImplementedError("load_header")

    def write_header(self, fd):
        raise NotImplementedError("write_header")

    def load_item(self, i):
        raise NotImplementedError("load_item")

    def write_entry(self, fd, op, key, value):
        raise NotImplementedError("write_entry")

    def load_entry(self, line):
        raise NotImplementedError("load_entry")


class StandardHeader(object):
    def dump_header(self, d):
        return " ".join("=".join(i) for i in d.items())

    def load_header(self, s):
        if not s:
            return None

        return dict(s.split("=", 1) for s in s.split(" "))


class BaseBlock(StandardHeader, Block):
    SPACE = " "
    DELIM = "\n"
    VERSION = None

    def __init__(self, encoding='utf-8'):
        self._encoding = encoding

    def header(self):
        return {'version': self.VERSION}

    def load_item(self, i):
        key, value = i
        return (self.load_key(key), self.load_value(value))

    def write_header(self, fd):
        fd.write(self.dump_header(self.header()) + self.DELIM)


class V1Block(BaseBlock):
    VERSION = '1.0'

    load_value = staticmethod(json_load)
    dump_value = staticmethod(json_dump)

    def write_entry(self, fd, op, key, value):
        key = self.dump_key(key).encode(self._encoding)
        value = self.dump_value(value).encode(self._encoding)
        key_length = str(len(key))
        entry = self.SPACE.join((op, key_length, key, value)) + self.DELIM
        fd.write(entry)

    def load_entry(self, line):
        op, key_length, b = line.split(self.SPACE, 2)
        key_length = int(key_length)
        key = b[:key_length].decode(self._encoding)
        value = b[key_length + 1:-1].decode(self._encoding)
        return op, key, value


class V2Block(BaseBlock):
    VERSION = '2.0'

    load_key = staticmethod(json_load)
    dump_key = staticmethod(json_dump)

    load_value = staticmethod(json_load)
    dump_value = staticmethod(json_dump)

    def write_entry(self, fd, op, key, value):
        key = self.dump_key(key)
        value = self.dump_value(value)
        entry = self.SPACE.join((op, key, value)) + self.DELIM
        fd.write(entry)

    def load_entry(self, line):
        op, key, value = line.split(self.SPACE, 2)
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
                op, key, value = self._block.load_entry(line)
                yield op, key, value
        finally:
            self._fd.close()
            self._fd = None

    def load_item(self, i):
        return self._block.load_item(i)


class FilesystemDriver(object):
    block_formats = {
        V1Block.VERSION: V1Block,
        V2Block.VERSION: V2Block,
    }

    default_block = V2Block

    def __init__(self, path, driver_open=open, encoding='utf-8'):
        self._path = path
        self._compact_path = os.path.join(
            os.path.dirname(path), u".{0}.compact".format(
                os.path.basename(path)))
        self._driver_open = driver_open
        self._encoding = encoding
        self._block = self.default_block(encoding=self._encoding)

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
                self._block.write_header(fd)

            for op, key, value in log:
                self._block.write_entry(fd, op, key, value)

    def createyielder(self):
        fd = self._driver_open(self._path, 'a+')

        header_line = fd.readline()

        if header_line:
            header = self._block.load_header(header_line.strip())
        else:
            header = dict(version=self.default_block.VERSION)

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

    def __init__(self, driver):
        self._driver = driver
        self._log = list()
        self._stats = None

    def read(self):
        self._stats, data = self._read_cache(self._driver.createyielder())
        return data

    def stats(self):
        return self._stats

    def _read_cache(self, yielder):
        data = dict()

        # generate statistics to decide if we should autocompact the database.
        clears = 0
        deletions = 0

        for op, key, value in yielder.readblocks():
            if op == self.DELETION:
                del data[key]
                deletions += 1
                continue

            if op == self.CLEAR:
                data.clear()
                clears += 1
                continue

            if op == self.VALUE:
                data[key] = value
                continue

            raise Exception("unknown operation '{0}'".format(op))

        data = dict(yielder.load_item(i) for i in data.items())

        stats = DictStorageStats(
            clears=clears,
            deletions=deletions,
            noops=deletions + clears)

        return stats, data

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

        stats = self._stats

        self._stats = DictStorageStats(
            clears=0, deletions=0, noops=0)

        return stats

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
        key, value = dict.popitem(self)
        self._storage.delitem(key)
        return key, value

    def list_append(self, key, data):
        array = self.get(key, [])
        array.append(data)
        self.__setitem__(key, array)

    def list_remove(self, key, data):
        array = filter(lambda v: v != data, self.get(key, []))
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

    storage = DictStorage(driver_instance)

    cache = storage.read()

    stats = storage.stats()

    db = impl(storage, cache)

    # noops means non-operations, basically operations that does not contribute
    # to the final structure of the data.
    #
    # if we hit a limit here, we should cleanup, otherwise it's a waste of
    # space.
    if stats.noops > compaction_limit:
        db.compact()

    yield db
    storage.commit()
