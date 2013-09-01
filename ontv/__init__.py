import os
import argparse
import logging
import blessings
import random
import contextlib

from .api import TheTVDBApi
from .action.sync import setup as sync_setup, action as sync_action
from .action.search import setup as search_setup
from .action.add import setup as add_setup
from .action.remove import setup as remove_setup
from .action.list import setup as list_setup
from .action.show import setup as show_setup
from .action.mark import setup as mark_setup
from .action.next import setup as next_setup
from .action.compact import setup as compact_setup

from .dao import SeriesDAO

from .utils import write_yaml
from .utils import read_yaml

from .database import open_database

__version__ = "0.7.10"

log = logging.getLogger(__name__)

LOGGING_FORMAT = "%(asctime)s %(levelname)7s %(message)s"

TEMPLATE_CONFIGURATION = {
    "api_key": None,
    "language": "en",
}


class ColorScheme(object):
    def __init__(self, term, colors=dict()):
        self.__dict__['_term'] = term
        self.__dict__['_colors'] = dict(colors)

    def __setattr__(self, key, name):
        self._colors[key] = name

    def __getattr__(self, key):
        name = self._colors.get(key)

        if not name:
            raise KeyError("Color not defined for '{0}'".format(key))

        color = getattr(self._term, name, None)

        if not color:
            raise KeyError("Missing color '{0}'".format(name))

        return color


def setup_parser(parser):
    parser.add_argument(
        '--libdir', '-d',
        metavar="<directory>",
        help="Library directory, defaults to $HOME/.ontv.",
        default=None,
    )

    parser.add_argument(
        '--loglevel', '-l',
        metavar="<level>",
        help="Logging level to use.",
        default=logging.WARNING,
    )

    parser.add_argument(
        '--debug',
        dest='loglevel',
        help="Use a debugging log level.",
        action='store_const',
        const=logging.DEBUG,
    )

    parser.add_argument(
        '--apikey',
        dest='api_key',
        metavar="<key>",
        help="API key to use",
        default=None,
    )

    parser.add_argument(
        '--language',
        dest='language',
        metavar="<language>",
        help="Language to use",
        default=None,
    )

    subparsers = parser.add_subparsers()

    sync_parser = subparsers.add_parser(
        "sync",
        help="Synchronize local database.",
    )
    sync_setup(sync_parser)

    search_parser = subparsers.add_parser(
        "search",
        help="Search for tv series.",
    )
    search_setup(search_parser)

    add_parser = subparsers.add_parser(
        "add",
        help="Add tv series.",
    )
    add_setup(add_parser)

    remove_parser = subparsers.add_parser(
        "remove",
        help="Remove tv series.",
    )
    remove_setup(remove_parser)

    list_parser = subparsers.add_parser(
        "list",
        help="List tv series you are watching.",
    )
    list_setup(list_parser)

    show_parser = subparsers.add_parser(
        "show",
        help="Show episodes in a series.",
    )
    show_setup(show_parser)

    mark_parser = subparsers.add_parser(
        "mark",
        help="Mark an episode as watched.",
    )
    mark_setup(mark_parser)

    next_parser = subparsers.add_parser(
        "next",
        help="Show the next episode to watch.",
    )
    next_setup(next_parser)

    compact_parser = subparsers.add_parser(
        "compact",
        help="Make the local database smaller.",
    )
    compact_setup(compact_parser)


def setup_ns(ns):
    home = os.environ.get("HOME")

    if ns.libdir is None:
        if not home:
            raise Exception("missing environment variable: HOME")

        ns.libdir = os.path.join(home, '.ontv')

    ns.mirrors_path = os.path.join(ns.libdir, 'mirrors.yaml')
    ns.languages_path = os.path.join(ns.libdir, 'languages.yaml')
    ns.config_path = os.path.join(ns.libdir, 'config.yaml')
    ns.db_path = os.path.join(ns.libdir, 'db')
    ns.series_db_path = os.path.join(ns.libdir, 'series')
    ns.episodes_db_path = os.path.join(ns.libdir, 'episodes')
    ns.watched_db_path = os.path.join(ns.libdir, 'watched')

    directories = [
        ns.libdir,
    ]

    for directory in directories:
        if not os.path.isdir(directory):
            log.info("Creating directory {0}".format(directory))
            os.mkdir(directory)

    ns.t = blessings.Terminal()

    if os.path.isfile(ns.config_path):
        log.debug("Loading configuration from {0}".format(ns.config_path))

        with open(ns.config_path) as fp:
            for key, value in read_yaml(fp).items():
                setattr(ns, key, value)
    else:
        log.info("Creating default configuration {0}".format(ns.config_path))

        with open(ns.config_path, 'w') as fp:
            write_yaml(fp, TEMPLATE_CONFIGURATION)

    if os.path.isfile(ns.mirrors_path):
        log.debug("Loading mirrors from {0}".format(ns.mirrors_path))

        with open(ns.mirrors_path) as fp:
            ns.mirrors = read_yaml(fp)
    else:
        ns.mirrors = []

    if os.path.isfile(ns.languages_path):
        log.debug("Loading mirrors from {0}".format(ns.languages_path))

        with open(ns.languages_path) as fp:
            ns.languages = read_yaml(fp)
    else:
        ns.languages = []

    if ns.languages:
        ns.abbrev_languages = [l['abbreviation'] for l in ns.languages]
    else:
        ns.abbrev_languages = []

    if ns.mirrors:
        ns.base_url = random.choice(ns.mirrors)['mirrorpath']
        log.debug("Picked mirror: {0}".format(ns.base_url))
    else:
        ns.base_url = None

    ns.api = TheTVDBApi(ns.api_key, base_url=ns.base_url)

    if ns.abbrev_languages and ns.language:
        if ns.language not in ns.abbrev_languages:
            raise Exception(
                "Language not valid, must be one of {0}".format(
                    ", ".join(ns.abbrev_languages)))
    else:
        ns.language = None

    ns.is_synced = bool(ns.abbrev_languages)

    ns.C = ColorScheme(ns.t)
    ns.C.range_before = 'green'
    ns.C.range_inside = 'yellow'
    ns.C.range_outside = 'red'
    ns.C.all_seen = 'magenta'
    ns.C.warning = 'red'
    ns.C.title = 'bold_magenta'


def main(args):
    parser = argparse.ArgumentParser(version="ontv " + __version__)

    setup_parser(parser)

    ns = parser.parse_args(args)

    logging.basicConfig(format=LOGGING_FORMAT, level=ns.loglevel)

    setup_ns(ns)

    databases = contextlib.nested(
        open_database(ns.db_path),
        open_database(ns.series_db_path),
        open_database(ns.episodes_db_path),
        open_database(ns.watched_db_path),
    )

    with databases as (db, series_db, episodes_db, watched_db):
        ns.databases = {
            "db": db,
            "series": series_db,
            "episodes": episodes_db,
            "watched": watched_db,
        }

        ns.series = SeriesDAO(db, series_db, episodes_db, watched_db)

        if not ns.is_synced and ns.action != sync_action:
            print ns.t.bold_red("Your first action should be 'sync'")
            return 1

        return ns.action(ns)
