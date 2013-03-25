from ..utils import with_series
from ..utils import local_series_finder


@with_series(local_series_finder)
def action(ns, series):
    ns.series.remove(series)
    print ns.term.green("removed: {0}".format(series['series_name']))
    return 0


def setup(parser):
    parser.add_argument(
        "series_id",
        metavar="<id|name>",
        help="The id of the series to remove",
    )

    parser.set_defaults(action=action)