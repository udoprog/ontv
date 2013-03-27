from ..utils import api_series_finder
from ..utils import with_resource


@with_resource(api_series_finder)
def action(ns, series):
    if ns.series.has_series(series):
        print ns.t.bold_red(
            "already exists: {0}".format(
                series['series_name']))
        return 0

    ns.series.add(series)

    print ns.t.bold_green(u"added: {0}".format(series['series_name']))
    return 0


def setup(parser):
    parser.add_argument(
        "series_query",
        metavar="<id|name>",
        help="The id of the series to add",
    )

    parser.set_defaults(action=action)
