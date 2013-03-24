from ..utils import api_series_finder
from ..utils import with_series


@with_series(api_series_finder)
def action(ns, series):
    if ns.series.has_series(series):
        print ns.term.bold_red(
            "already exists: {0}".format(
                series['series_name']))
        return 0

    ns.series.add(series)

    print ns.term.bold_green(u"added: {0}".format(series['series_name']))
    return 0


def setup(parser):
    parser.add_argument(
        "series_id",
        metavar="<id|name>",
        help="The id of the series to add",
    )

    parser.set_defaults(action=action)
