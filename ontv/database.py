import json
import contextlib


class FilesystemDriver(object):
    SPACE = " "
    DELIM = "\n"
    PUT = "+"
    REMOVE = "-"

    def __init__(self, path):
        self._path = path
        self._fd = None
        self._log = list()

    def open(self):
        self._fd = open(self._path, 'a+')

    def _load_block(self, v):
        if v == "":
            return None

        try:
            return json.loads(v)
        except:
            return None

    def _dump_block(self, v):
        if v is None:
            return ""

        try:
            return json.dumps(v)
        except:
            return ""

    def read_dict(self):
        if not self._fd:
            raise Exception("file not open")

        self._fd.seek(0)

        data = dict()

        for i, line in enumerate(self._fd):
            op, ident, block = line.split(self.SPACE, 2)

            if op == self.REMOVE:
                data.pop(ident, None)
                continue

            if op == self.PUT:
                data[ident] = block
                continue

            raise Exception("unknown operation '{0}'".format(op))

        return dict((k, self._load_block(v)) for k, v in data.items())

    def _close(self, flush=True):
        if not self._fd:
            return

        self._fd.close()
        self._fd = None

    def commit(self):
        self._flush_log()
        self._close()

    def rollback(self):
        self._close()

    def _flush_log(self):
        for op, ident, block in self._log:
            block = self._dump_block(block)
            self._fd.write(self.SPACE.join((op, ident, block)) + self.DELIM)

        self._log = list()

    def _validate_identifier(self, ident):
        if self.SPACE in ident:
            raise Exception("identifier may not contain a space")

        if self.DELIM in ident:
            raise Exception("identifier may not contain a newline")

    def put(self, ident, block):
        self._validate_identifier(ident)
        self._log.append((self.PUT, ident, block))

    def remove(self, ident):
        self._validate_identifier(ident)
        self._log.append((self.REMOVE, ident, None))


class Database(object):
    def __init__(self, driver):
        self._driver = driver
        self._cache = self._driver.read_dict()

    def __contains__(self, ident):
        return ident in self._cache

    def get(self, ident, default=None):
        return self._cache.get(ident, default)

    def put(self, ident, data):
        self._cache[ident] = data
        self._driver.put(ident, data)

    def remove(self, ident):
        try:
            del self._cache[ident]
            self._driver.remove(ident)
        except KeyError:
            pass

    def keys(self):
        return self._cache.keys()

    def values(self):
        return self._cache.values()

    def list_append(self, ident, data):
        array = list(self.get(ident, []))
        array.append(data)
        self.put(ident, array)

    def list_remove(self, ident, data):
        array = self.get(ident, [])
        array.remove(data)
        self.put(ident, array)


class SetDatabase(object):
    def __init__(self, driver):
        self._driver = driver
        self._cache = set(self._driver.read_dict().keys())

    def __contains__(self, ident):
        return ident in self._cache

    def add(self, ident):
        if ident in self._cache:
            return

        self._cache.add(ident)
        self._driver.put(ident, None)

    def remove(self, ident):
        try:
            self._cache.remove(ident)
            self._driver.remove(ident)
        except KeyError:
            pass

    def keys(self):
        return list(self._cache)


@contextlib.contextmanager
def open_database(path, database=Database):
    driver = FilesystemDriver(path)
    driver.open()

    db = database(driver)

    try:
        yield db
    except:
        driver.rollback()
        raise

    driver.commit()
