from ..utils import find_series


def action(ns):
    series_id = find_series(ns.term, ns.series.find_series, ns.series_id)

    if series_id is None:
        print ns.term.bold_red(u"no such series: {0}".format(ns.series_id))
        return 0

    series = ns.series.get(series_id)

    if series is None:
        print ns.term.bold_red(u"no such series (id): {0}".format(series_id))
        return 0

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
