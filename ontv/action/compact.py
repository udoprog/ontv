def action(ns):
    print ns.term.bold_magenta(u"Compacting local databases")
    print u""

    for name, db in ns.databases.items():
        print ns.term.cyan(u"Compacting database: {0}".format(name))
        statistics = db.compact()
        print ns.term.cyan(u"  pruned {0[nops]} NO-OP entries".format(
            statistics))

    return 0


def setup(parser):
    parser.set_defaults(action=action)
