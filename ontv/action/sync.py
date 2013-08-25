from ..utils import write_yaml
from ..utils import with_resource
from ..utils import local_series_finder


@with_resource(local_series_finder)
def action(ns, series):
    print ns.t.bold_magenta(u"Synchronizing local database")
    print u""

    if not ns.api.is_authenticated():
        print ns.t.bold_red(
            "Cannot fully synchronize, api is not authenticated with a key!")
        print (
            ns.t.red("Set a value to the 'api_key' option in ") +
            ns.t.bold_red(ns.config_path)
        )
        print (
            ns.t.red("If you don't have an API key, register yourself at ") +
            ns.t.bold_red("http://thetvdb.com/?tab=register")
        )
        return 0

    if not ns.languages:
        with open(ns.languages_path, 'w') as fp:
            write_yaml(fp, ns.api.languages())

    if not ns.mirrors:
        print ns.t.bold_green("Downloading mirrors to {0}".format(
            ns.mirrors_path))

        with open(ns.mirrors_path, 'w') as fp:
            write_yaml(fp, ns.api.mirrors())

    if not series:
        series = ns.series.list_series()
    else:
        series = [series]

    for s in series:
        print ns.t.bold_cyan(
            u"Syncing: {0}".format(s['series_name'])
        )

        s, episodes = ns.api.series_all(
            s['id'], ns.language)

        ns.series.set_episodes(s, episodes)

    return 0


def setup(parser):
    parser.add_argument(
        "series_query",
        metavar="<name|id>",
        nargs='?',
        default=None,
        help="The id or name of the series to sync.",
    )

    parser.set_defaults(action=action)
