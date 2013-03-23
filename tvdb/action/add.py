from ..utils import find_series


def action(ns):
    if ns.language is None:
        print ns.term.bold_red("language must be configured")
        return 1

    series_id = find_series(ns.term, ns.api.getseries, ns.series_id)

    if series_id is None:
        print ns.term.bold_red(
            u"could not add: {0}".format(ns.series_id))
        return 1

    series = ns.api.series(series_id, ns.language)

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
