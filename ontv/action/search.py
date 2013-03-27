from ..format import print_series


def action(ns):
    print ns.t.bold_magenta(u"Results from searching online database")
    print u""

    if not ns.api.is_authenticated():
        print ns.t.bold_red(u"Not authenticated")
        return 1

    series = ns.api.getseries(ns.query)

    if not series:
        print ns.t.bold_red("No series matching '{0}'".format(ns.query))
        return 1

    if ns.limit:
        series = series[:ns.limit]

    for s in series:
        print_series(ns.t, s)


def setup(parser):
    parser.add_argument(
        "query",
        metavar="<query>",
    )

    parser.add_argument(
        "--limit",
        metavar="<number>",
        help="Limit the amount of search results displayed, default: 5.",
        type=int,
        default=5)

    parser.set_defaults(action=action)
