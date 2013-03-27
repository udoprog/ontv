def action(ns):
    print ns.t.bold_magenta(u"Compacting local databases")
    print u""

    for name, db in ns.databases.items():
        print ns.t.cyan(u"Compacting database: {0}".format(name))
        stats = db.compact()
        print ns.t.cyan(u"  pruned {0.noops} non-operation entries".format(
            stats))

    return 0


def setup(parser):
    parser.set_defaults(action=action)
